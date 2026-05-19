use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::{check_ty, extension::Extension, ChunkHeader, ChunkType};

use super::extension::UserData;
#[derive(Debug, Serialize, Clone, Default)]
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
    /// Baked lightmap atlas texture parsed from the material's `0x120`
    /// EXTENSION chunk (`extension::LightMapPlugin`). The texture's
    /// `name` is the atlas (e.g. `"Cylinder1746LightingMap"`); pair it
    /// with the material's primary `texture` (the diffuse, e.g.
    /// `"fz01"`) and sample the atlas with the geometry's secondary UV
    /// set. `None` for materials without a baked lightmap (typical for
    /// actors and effects that don't ship through the scene BSP).
    pub lightmap: Option<Texture>,
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
        let lightmap = lightmap_from_extensions(&extensions);

        Ok(Self {
            unknown,
            color,
            unknown2,
            texture,
            ambient,
            specular,
            diffuse,
            userdata_name,
            lightmap,
        })
    }

    /// Convenience accessor: returns just the lightmap atlas's texture
    /// name when the material has one. Kept for call sites that don't
    /// need the full `Texture` (filter mode, address modes, mask).
    pub fn lightmap_name(&self) -> Option<&str> {
        self.lightmap.as_ref().map(|t| t.name.as_str())
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

/// Pick the lightmap atlas `Texture` out of a material's EXTENSION
/// block. PAL4 materials ship at most one `0x120` plugin per material;
/// if multiple are present (not observed in any shipped BSP) we use
/// the first.
fn lightmap_from_extensions(extensions: &[Extension]) -> Option<Texture> {
    for ext in extensions {
        if let Extension::LightMapPlugin(plugin) = ext {
            if let Some(tex) = &plugin.texture {
                return Some(tex.clone());
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
        assert!(mat.lightmap.is_none());
    }

    /// Synthesize a PAL4-style material whose EXTENSION block contains
    /// a custom `0x120` lightmap plugin with the observed 6-u32
    /// preamble (`4, 4, 1, 3, 1, 6`) followed by a standard RW
    /// TEXTURE chunk naming `"Cylinder1746LightingMap"`. Verifies
    /// `Material::lightmap_name` extracts it.
    #[test]
    fn material_extracts_lightmap_name_from_0x120_plugin() {
        // Inner TEXTURE chunk: STRUCT(filter modes) + STRING(name) +
        // STRING(mask="") + EXTENSION(empty).
        let mut tex_body: Vec<u8> = Vec::new();
        write_header(&mut tex_body, ChunkType::STRUCT.0, 4);
        tex_body.extend(&0x1106u32.to_le_bytes()); // filter/addr modes
        let name = b"Cylinder1746LightingMap\0";
        write_header(&mut tex_body, ChunkType::STRING.0, name.len() as u32);
        tex_body.extend(name);
        // mask name (empty), 4-byte aligned to "\0\0\0\0"
        write_header(&mut tex_body, ChunkType::STRING.0, 4);
        tex_body.extend(&[0u8; 4]);
        write_header(&mut tex_body, ChunkType::EXTENSION.0, 0);

        let mut texture_chunk: Vec<u8> = Vec::new();
        write_header(&mut texture_chunk, ChunkType::TEXTURE.0, tex_body.len() as u32);
        texture_chunk.extend(&tex_body);

        // 0x120 body: 6-u32 preamble + TEXTURE chunk.
        let mut plugin_body: Vec<u8> = Vec::new();
        for v in [4u32, 4, 1, 3, 1, 6] {
            plugin_body.extend(&v.to_le_bytes());
        }
        plugin_body.extend(&texture_chunk);

        let mut plugin_chunk: Vec<u8> = Vec::new();
        write_header(&mut plugin_chunk, 0x120, plugin_body.len() as u32);
        plugin_chunk.extend(&plugin_body);

        // MATERIAL: STRUCT (untextured) + EXTENSION containing the
        // 0x120 plugin only.
        let mut body: Vec<u8> = Vec::new();
        write_header(&mut body, ChunkType::STRUCT.0, 7 * 4);
        body.extend(&0u32.to_le_bytes());
        body.extend(&0u32.to_le_bytes());
        body.extend(&0u32.to_le_bytes());
        body.extend(&0u32.to_le_bytes()); // textured=false
        body.extend(&0f32.to_le_bytes());
        body.extend(&0f32.to_le_bytes());
        body.extend(&0f32.to_le_bytes());
        write_header(&mut body, ChunkType::EXTENSION.0, plugin_chunk.len() as u32);
        body.extend(&plugin_chunk);

        let mut cursor = Cursor::new(body);
        let mat = Material::read(&mut cursor).expect("parse material");
        assert_eq!(
            mat.lightmap_name(),
            Some("Cylinder1746LightingMap")
        );
        let lm = mat.lightmap.as_ref().expect("typed lightmap parsed");
        // Filter / address-mode lanes come from the nested TEXTURE
        // chunk's STRUCT preamble (`0x1106` in the fixture).
        assert_eq!(lm.filter_mode, 0x1106 & 0xff);
        assert_eq!(lm.address_mode_u, (0x1106 & 0x0f00) >> 8);
        assert_eq!(lm.address_mode_v, (0x1106 & 0xf000) >> 12);
        assert_eq!(lm.mask_name, "");
    }

    /// Verify the typed `LightMapPlugin` survives a non-default filter
    /// / address mode in the nested TEXTURE chunk so we know the
    /// sampler metadata round-trips for downstream callers.
    #[test]
    fn material_lightmap_carries_sampler_metadata() {
        // Filter = 4 (linear-mip-nearest), addr_u = 3 (clamp), addr_v
        // = 1 (repeat) — encoded as `(v << 12) | (u << 8) | filter`.
        let modes: u32 = (1u32 << 12) | (3u32 << 8) | 0x04;
        let mut tex_body: Vec<u8> = Vec::new();
        write_header(&mut tex_body, ChunkType::STRUCT.0, 4);
        tex_body.extend(&modes.to_le_bytes());
        let name = b"AtlasA\0";
        write_header(&mut tex_body, ChunkType::STRING.0, name.len() as u32);
        tex_body.extend(name);
        write_header(&mut tex_body, ChunkType::STRING.0, 4);
        tex_body.extend(&[0u8; 4]);
        write_header(&mut tex_body, ChunkType::EXTENSION.0, 0);

        let mut texture_chunk: Vec<u8> = Vec::new();
        write_header(&mut texture_chunk, ChunkType::TEXTURE.0, tex_body.len() as u32);
        texture_chunk.extend(&tex_body);

        let mut plugin_body: Vec<u8> = Vec::new();
        for v in [4u32, 4, 1, 3, 1, 6] {
            plugin_body.extend(&v.to_le_bytes());
        }
        plugin_body.extend(&texture_chunk);

        let mut plugin_chunk: Vec<u8> = Vec::new();
        write_header(&mut plugin_chunk, 0x120, plugin_body.len() as u32);
        plugin_chunk.extend(&plugin_body);

        let mut body: Vec<u8> = Vec::new();
        write_header(&mut body, ChunkType::STRUCT.0, 7 * 4);
        for _ in 0..4 { body.extend(&0u32.to_le_bytes()); }
        body.extend(&0f32.to_le_bytes());
        body.extend(&0f32.to_le_bytes());
        body.extend(&0f32.to_le_bytes());
        write_header(&mut body, ChunkType::EXTENSION.0, plugin_chunk.len() as u32);
        body.extend(&plugin_chunk);

        let mut cursor = Cursor::new(body);
        let mat = Material::read(&mut cursor).expect("parse material");
        let lm = mat.lightmap.as_ref().expect("lightmap parsed");
        assert_eq!(lm.name, "AtlasA");
        assert_eq!(lm.filter_mode, 4);
        assert_eq!(lm.address_mode_u, 3);
        assert_eq!(lm.address_mode_v, 1);
    }

    /// Regression for the real-world panic observed loading PAL4
    /// scene `Q01.bsp`: a `0x120` chunk whose preamble is NOT the
    /// synthetic `[4,4,1,3,1,6]` u32 layout but contains arbitrary
    /// bytes (and possibly an embedded `7F`-looking byte that the
    /// previous strict parser mis-read as a TEXTURE-chunk type and
    /// then failed `check_ty!(_, TEXTURE)`). The current scanner
    /// must skip the preamble and still find the real TEXTURE chunk.
    #[test]
    fn material_lightmap_robust_to_preamble_drift() {
        // Inner TEXTURE chunk.
        let mut tex_body: Vec<u8> = Vec::new();
        write_header(&mut tex_body, ChunkType::STRUCT.0, 4);
        tex_body.extend(&0x1106u32.to_le_bytes());
        let name = b"AtlasZ\0";
        write_header(&mut tex_body, ChunkType::STRING.0, name.len() as u32);
        tex_body.extend(name);
        write_header(&mut tex_body, ChunkType::STRING.0, 4);
        tex_body.extend(&[0u8; 4]);
        write_header(&mut tex_body, ChunkType::EXTENSION.0, 0);
        let mut texture_chunk: Vec<u8> = Vec::new();
        write_header(&mut texture_chunk, ChunkType::TEXTURE.0, tex_body.len() as u32);
        texture_chunk.extend(&tex_body);

        // Arbitrary 8-u32 preamble whose lanes include 0x7F (the
        // value the production panic logged as the would-be TEXTURE
        // chunk type when the parser assumed a fixed 6-u32 preamble).
        let mut plugin_body: Vec<u8> = Vec::new();
        for v in [0u32, 0x7F, 1, 0, 3, 1, 0, 0] {
            plugin_body.extend(&v.to_le_bytes());
        }
        plugin_body.extend(&texture_chunk);

        let mut plugin_chunk: Vec<u8> = Vec::new();
        write_header(&mut plugin_chunk, 0x120, plugin_body.len() as u32);
        plugin_chunk.extend(&plugin_body);

        let mut body: Vec<u8> = Vec::new();
        write_header(&mut body, ChunkType::STRUCT.0, 7 * 4);
        for _ in 0..4 { body.extend(&0u32.to_le_bytes()); }
        body.extend(&0f32.to_le_bytes());
        body.extend(&0f32.to_le_bytes());
        body.extend(&0f32.to_le_bytes());
        write_header(&mut body, ChunkType::EXTENSION.0, plugin_chunk.len() as u32);
        body.extend(&plugin_chunk);

        let mut cursor = Cursor::new(body);
        let mat = Material::read(&mut cursor).expect("parse must not panic on preamble drift");
        let lm = mat.lightmap.as_ref().expect("texture still recovered");
        assert_eq!(lm.name, "AtlasZ");
    }

    /// A `0x120` chunk whose body has no embedded TEXTURE chunk must
    /// not abort the BSP loader — `Material::lightmap` simply becomes
    /// `None`.
    #[test]
    fn material_lightmap_missing_texture_is_none_not_error() {
        let plugin_body: Vec<u8> = vec![0xAA; 24];
        let mut plugin_chunk: Vec<u8> = Vec::new();
        write_header(&mut plugin_chunk, 0x120, plugin_body.len() as u32);
        plugin_chunk.extend(&plugin_body);

        let mut body: Vec<u8> = Vec::new();
        write_header(&mut body, ChunkType::STRUCT.0, 7 * 4);
        for _ in 0..4 { body.extend(&0u32.to_le_bytes()); }
        body.extend(&0f32.to_le_bytes());
        body.extend(&0f32.to_le_bytes());
        body.extend(&0f32.to_le_bytes());
        write_header(&mut body, ChunkType::EXTENSION.0, plugin_chunk.len() as u32);
        body.extend(&plugin_chunk);

        let mut cursor = Cursor::new(body);
        let mat = Material::read(&mut cursor).expect("parse");
        assert!(mat.lightmap.is_none());
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
