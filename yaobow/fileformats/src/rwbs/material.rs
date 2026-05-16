use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::{check_ty, extension::Extension, ChunkHeader, ChunkType};

use super::extension::UserData;#[derive(Debug, Serialize, Clone, Default)]
pub struct Texture {
    pub filter_mode: u32,
    pub address_mode_u: u32,
    pub address_mode_v: u32,
    pub name: String,
    pub mask_name: String,
}

impl Texture {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRUCT);

        let modes = cursor.read_u32_le()?;

        let filter_mode = modes & _private::TEXTURE_FILTER_MODE_MASK;
        let address_mode_u = (modes & _private::TEXTURE_ADDRESS_MODE_U_MASK) >> 8;
        let address_mode_v = (modes & _private::TEXTURE_ADDRESS_MODE_V_MASK) >> 12;

        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRING);

        let name = cursor.read_gbk_string(header.length as usize)?;

        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRING);

        let mask_name = cursor.read_gbk_string(header.length as usize)?;

        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::EXTENSION);

        cursor.skip(header.length as usize)?;

        Ok(Self {
            filter_mode,
            address_mode_u,
            address_mode_v,
            name,
            mask_name,
        })
    }
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct Material {
    pub unknown: u32,
    pub color: u32,
    pub unknown2: u32,
    pub texture: Option<Texture>,
    pub ambient: f32,
    pub specular: f32,
    pub diffuse: f32,
    /// Value of the material-level `PLUGIN_USERDATA name` entry when one
    /// is present in the EXTENSION chunk. PAL4 water materials stamp the
    /// UV-animation name here (e.g. `"Material #6662438"`); this is the
    /// lookup key into the scene's sibling `.uva` dictionary
    /// (`crate::rwbs::uva::UvAnimDict::find`). `None` for materials
    /// without the entry (the typical case for non-water materials).
    pub userdata_name: Option<String>,
}

impl Material {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRUCT);

        let unknown = cursor.read_u32_le()?;
        let color = cursor.read_u32_le()?;
        let unknown2 = cursor.read_u32_le()?;
        let textured = cursor.read_u32_le()?;
        let ambient = cursor.read_f32::<LittleEndian>()?;
        let specular = cursor.read_f32::<LittleEndian>()?;
        let diffuse = cursor.read_f32::<LittleEndian>()?;
        let mut texture = None;

        if textured != 0 {
            let _header = ChunkHeader::read(cursor)?;
            texture = Some(Texture::read(cursor)?);
        }

        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::EXTENSION);

        // Parse the EXTENSION body and extract the userdata `name` link.
        // Materials carry no per-vertex skin data, so the `vertices_count`
        // threaded through `Extension::read_data` only as `SkinPlugin`
        // context isn't relevant here. The parsed extensions are
        // discarded after the name extraction so `Material` stays `Clone`
        // (the Extension enum isn't Clone-able without a much larger
        // patch and isn't needed for any downstream consumer today).
        let extensions = Extension::read_data(cursor, header.length, 0)?;
        let userdata_name = userdata_name_from_extensions(&extensions);

        Ok(Self {
            unknown,
            color,
            unknown2,
            texture,
            ambient,
            specular,
            diffuse,
            userdata_name,
        })
    }
}

fn userdata_name_from_extensions(extensions: &[Extension]) -> Option<String> {
    for ext in extensions {
        if let Extension::UserDataPlugin(udp) = ext {
            if let Some(items) = udp.data().get("name") {
                if let Some(UserData::String(s)) = items.first() {
                    return Some(s.clone());
                }
            }
        }
    }
    None
}

pub fn read_material_list_header(cursor: &mut dyn Read) -> anyhow::Result<ChunkHeader> {
    let header = ChunkHeader::read(cursor)?;
    check_ty!(header.ty, ChunkType::MATERIAL_LIST);

    Ok(header)
}

pub fn read_material_list(cursor: &mut dyn Read) -> anyhow::Result<Vec<Material>> {
    let header = ChunkHeader::read(cursor)?;
    check_ty!(header.ty, ChunkType::STRUCT);

    let mut material_vec = vec![];

    let material_count = cursor.read_u32_le()?;
    if material_count > 0 {
        // The "material index table" preceding the MATERIAL chunks per the
        // RW 3.4+ spec:
        //   idx == -1 : the next slot has its own MATERIAL chunk to parse.
        //   idx >=  0 : the slot reuses (shares) material_vec[idx]; no chunk
        //               is emitted on disk for this entry.
        // Some games (notably PAL5) build BSPs that use shared indices
        // heavily, so we cannot just skip the table and read a MATERIAL
        // chunk per slot. However, other titles (PAL3/PAL4) emit a DWORD
        // per slot here whose contents are NOT share indices, with a full
        // run of N MATERIAL chunks following regardless. Detect which
        // dialect we're looking at by validating the share-format
        // invariants:
        //   * indices[0] must be -1 (you cannot share before any slot is
        //     defined);
        //   * for i > 0, indices[i] is either -1 or a back-reference to a
        //     strictly earlier slot (0 <= indices[i] < i).
        // If those invariants hold, treat the table as share indices;
        // otherwise fall back to the legacy "read N MATERIAL chunks"
        // behavior so we don't desync the cursor.
        let mut indices = Vec::with_capacity(material_count as usize);
        for _ in 0..material_count {
            indices.push(cursor.read_i32::<LittleEndian>()?);
        }

        let is_share_format = indices[0] == -1
            && indices
                .iter()
                .enumerate()
                .all(|(i, &idx)| idx == -1 || (idx >= 0 && (idx as usize) < i));

        if is_share_format {
            for idx in indices {
                if idx < 0 {
                    let header = ChunkHeader::read(cursor)?;
                    check_ty!(header.ty, ChunkType::MATERIAL);
                    material_vec.push(Material::read(cursor)?);
                } else {
                    let shared = material_vec[idx as usize].clone();
                    material_vec.push(shared);
                }
            }
        } else {
            // Legacy / non-share dialect: one MATERIAL chunk per slot.
            for _ in 0..material_count {
                let header = ChunkHeader::read(cursor)?;
                check_ty!(header.ty, ChunkType::MATERIAL);
                material_vec.push(Material::read(cursor)?);
            }
        }
    }

    Ok(material_vec)
}

mod _private {
    pub const TEXTURE_FILTER_MODE_MASK: u32 = 0x000000ff;
    pub const TEXTURE_ADDRESS_MODE_U_MASK: u32 = 0x00000f00;
    pub const TEXTURE_ADDRESS_MODE_V_MASK: u32 = 0x0000f000;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn write_header(buf: &mut Vec<u8>, ty: u32, length: u32) {
        buf.extend(&ty.to_le_bytes());
        buf.extend(&length.to_le_bytes());
        buf.extend(&0u16.to_le_bytes()); // build
        buf.extend(&0u16.to_le_bytes()); // version
    }

    /// Encode a minimal non-textured MATERIAL chunk body (everything after
    /// the outer MATERIAL header).
    fn material_chunk(color: u32) -> Vec<u8> {
        let mut body = Vec::new();
        // inner STRUCT header: length = 7*4 (scalars) + 12 (empty EXTENSION header)
        let body_len: u32 = 7 * 4 + 12;
        write_header(&mut body, ChunkType::STRUCT.0, body_len);
        body.extend(&0u32.to_le_bytes()); // unknown
        body.extend(&color.to_le_bytes()); // color
        body.extend(&0u32.to_le_bytes()); // unknown2
        body.extend(&0u32.to_le_bytes()); // textured=false
        body.extend(&0f32.to_le_bytes()); // ambient
        body.extend(&0f32.to_le_bytes()); // specular
        body.extend(&0f32.to_le_bytes()); // diffuse
        write_header(&mut body, ChunkType::EXTENSION.0, 0);
        body
    }

    #[test]
    fn material_extracts_userdata_name_link_for_uva_lookup() {
        // Construct a MATERIAL with a single PLUGIN_USERDATA entry of
        // form `name = "Material #6662438"` — the exact shape PAL4 water
        // materials use to link to their `.uva` UV-animation.
        // PLUGIN_USERDATA body for one entry "name" → ["Material #6662438"]:
        //   u32 entry_count = 1
        //   entry:
        //     u32 name_len + bytes ("name")
        //     u32 type = 3 (string)
        //     u32 item_count = 1
        //     u32 str_len + bytes
        let value = b"Material #6662438\0";
        let mut udp_body: Vec<u8> = Vec::new();
        udp_body.extend(&1u32.to_le_bytes());
        udp_body.extend(&5u32.to_le_bytes());
        udp_body.extend(b"name\0");
        udp_body.extend(&3u32.to_le_bytes());
        udp_body.extend(&1u32.to_le_bytes());
        udp_body.extend(&(value.len() as u32).to_le_bytes());
        udp_body.extend(value);

        let mut udp_chunk: Vec<u8> = Vec::new();
        write_header(&mut udp_chunk, 0x11F, udp_body.len() as u32);
        udp_chunk.extend(&udp_body);

        let mut body: Vec<u8> = Vec::new();
        let struct_len: u32 = 7 * 4;
        write_header(&mut body, ChunkType::STRUCT.0, struct_len);
        body.extend(&0u32.to_le_bytes()); // unknown
        body.extend(&0u32.to_le_bytes()); // color
        body.extend(&0u32.to_le_bytes()); // unknown2
        body.extend(&0u32.to_le_bytes()); // textured=false
        body.extend(&0f32.to_le_bytes());
        body.extend(&0f32.to_le_bytes());
        body.extend(&0f32.to_le_bytes());
        write_header(&mut body, ChunkType::EXTENSION.0, udp_chunk.len() as u32);
        body.extend(&udp_chunk);

        let mut cursor = Cursor::new(body);
        let mat = Material::read(&mut cursor).expect("parse material");
        assert_eq!(mat.userdata_name.as_deref(), Some("Material #6662438"));
    }

    #[test]
    fn material_without_userdata_name_has_none_link() {
        let body = material_chunk(0);
        let mut cursor = Cursor::new(body);
        let mat = Material::read(&mut cursor).expect("parse material");
        assert!(mat.userdata_name.is_none());
    }

    #[test]
    fn material_list_share_index_reuses_prior_material() {
        let mut buf = Vec::new();
        // Outer STRUCT header (length field is unused by the parser).
        write_header(&mut buf, ChunkType::STRUCT.0, 0);
        // count = 2
        buf.extend(&2u32.to_le_bytes());
        // indices: [-1, 0] — second slot shares first.
        buf.extend(&(-1i32).to_le_bytes());
        buf.extend(&0i32.to_le_bytes());
        // Exactly one MATERIAL chunk follows.
        let mc = material_chunk(0xAABBCCDD);
        write_header(&mut buf, ChunkType::MATERIAL.0, mc.len() as u32);
        buf.extend(&mc);

        let mut cursor = Cursor::new(buf);
        let mats = read_material_list(&mut cursor).expect("parse");
        assert_eq!(mats.len(), 2);
        assert_eq!(mats[0].color, 0xAABBCCDD);
        assert_eq!(mats[1].color, 0xAABBCCDD);
        // Cursor must land exactly at end of input — proves we did not over-
        // or under-read the material list.
        assert_eq!(cursor.position(), cursor.get_ref().len() as u64);
    }

    #[test]
    fn material_list_legacy_dialect_reads_one_chunk_per_slot() {
        // Some titles (PAL3/PAL4) store a DWORD per slot here that does NOT
        // follow the RW share-index invariants and always emit one MATERIAL
        // chunk per slot. Index table starting with a non-(-1) value is the
        // detection signal for the legacy dialect.
        let mut buf = Vec::new();
        write_header(&mut buf, ChunkType::STRUCT.0, 0);
        buf.extend(&2u32.to_le_bytes());
        // Non-share-format leading DWORDs (e.g. zeros or sequence numbers).
        buf.extend(&0i32.to_le_bytes());
        buf.extend(&0i32.to_le_bytes());
        // Two MATERIAL chunks follow, one per slot.
        let mc0 = material_chunk(0x11223344);
        write_header(&mut buf, ChunkType::MATERIAL.0, mc0.len() as u32);
        buf.extend(&mc0);
        let mc1 = material_chunk(0x55667788);
        write_header(&mut buf, ChunkType::MATERIAL.0, mc1.len() as u32);
        buf.extend(&mc1);

        let mut cursor = Cursor::new(buf);
        let mats = read_material_list(&mut cursor).expect("parse");
        assert_eq!(mats.len(), 2);
        assert_eq!(mats[0].color, 0x11223344);
        assert_eq!(mats[1].color, 0x55667788);
        assert_eq!(cursor.position(), cursor.get_ref().len() as u64);
    }
}
