use std::{collections::HashMap, io::Read};

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use encoding::{DecoderTrap, types::Encoding};
use serde::Serialize;

use crate::rwbs::{ChunkHeader, ChunkType, check_ty};

use super::{Matrix44f, material::Texture, plugins::hanim::HAnimPlugin};

#[derive(Debug, Serialize)]
pub enum Extension {
    RightToRender(RightToRenderPlugin),
    HAnimPlugin(HAnimPlugin),
    SkinPlugin(SkinPlugin),
    UserDataPlugin(UserDataPlugin),
    NodeNamePlugin(NodeNamePlugin),
    BinMeshPlugin(BinMeshPlugin),
    /// PAL4's per-material lightmap plugin (RW chunk `0x120`). Carries
    /// the baked lightmap atlas as a standard RW `Texture` (the atlas
    /// texture name, e.g. `"Cylinder1746LightingMap"`, lives in
    /// `texture.name`). The plugin's body starts with a 6-u32 preamble
    /// — observed values `4, 4, 1, 3, 1, 6` in every shipped PAL4 BSP
    /// — preserved verbatim in `raw_preamble` so we can round-trip /
    /// validate later if a new dialect appears.
    LightMapPlugin(LightMapPlugin),
    UnknownPlugin(UnknownPlugin),
}

impl Extension {
    pub fn read_header(cursor: &mut dyn Read) -> anyhow::Result<ChunkHeader> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::EXTENSION);

        Ok(header)
    }

    pub fn read_data(
        cursor: &mut dyn Read,
        mut chunk_len: u32,
        vertices_count: u32,
    ) -> anyhow::Result<Vec<Self>> {
        let mut extensions = vec![];
        while chunk_len > 0 {
            let ext_header = ChunkHeader::read(cursor)?;
            chunk_len -= 12 + ext_header.length;

            let ext = match ext_header.ty {
                ChunkType::RIGHT_TO_RENDER => {
                    Extension::RightToRender(RightToRenderPlugin::read(cursor)?)
                }

                ChunkType::PLUGIN_HANIM => {
                    Extension::HAnimPlugin(HAnimPlugin::read(cursor, ext_header)?)
                }

                ChunkType::PLUGIN_SKIN => {
                    Extension::SkinPlugin(SkinPlugin::read(cursor, ext_header, vertices_count)?)
                }

                ChunkType::PLUGIN_NODENAME => {
                    Extension::NodeNamePlugin(NodeNamePlugin::read(cursor, ext_header)?)
                }

                ChunkType::PLUGIN_USERDATA => {
                    Extension::UserDataPlugin(UserDataPlugin::read(cursor, ext_header)?)
                }

                ChunkType::PLUGIN_BINMESH => {
                    Extension::BinMeshPlugin(BinMeshPlugin::read(cursor, ext_header)?)
                }

                ChunkType::PLUGIN_LIGHTMAP => {
                    Extension::LightMapPlugin(LightMapPlugin::read(cursor, ext_header)?)
                }

                _ => Extension::UnknownPlugin(UnknownPlugin::read(cursor, ext_header)?),
            };

            extensions.push(ext);
        }

        Ok(extensions)
    }

    pub fn read(cursor: &mut dyn Read, vertices_count: u32) -> anyhow::Result<Vec<Self>> {
        let header = Self::read_header(cursor)?;
        Self::read_data(cursor, header.length, vertices_count)
    }
}

#[derive(Debug, Serialize)]
pub struct RightToRenderPlugin {
    pub target_plugin_id: u32,
    pub param: u32,
}

impl RightToRenderPlugin {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let target_plugin_id = cursor.read_u32_le()?;
        let param = cursor.read_u32_le()?;
        Ok(Self {
            target_plugin_id,
            param,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct SkinPlugin {
    pub used_bones: Vec<u8>,
    pub bone_indices: Vec<[u8; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub matrix: Vec<Matrix44f>,
    unknown2: Vec<u8>,
}

impl SkinPlugin {
    pub fn read(
        cursor: &mut dyn Read,
        header: ChunkHeader,
        vertices_count: u32,
    ) -> anyhow::Result<Self> {
        let count = cursor.read_u32_le()?;
        let bone_count = count & 0xFF;

        let used_bone_count = (count & 0xFF00) >> 8;
        let used_bones = cursor.read_u8_vec(used_bone_count as usize)?;

        let mut bone_indices = vec![];
        for _ in 0..vertices_count {
            let indices = cursor.read_u8_vec(4)?;
            bone_indices.push(indices.try_into().unwrap());
        }

        let mut weights = vec![];
        for _ in 0..vertices_count {
            let weight = cursor.read_f32_vec(4)?;
            weights.push(weight.try_into().unwrap());
        }

        let mut matrix = vec![];
        for _ in 0..bone_count {
            let m = Matrix44f::read(cursor)?;
            matrix.push(m);
        }

        // The skin-plugin trailer length depends on the RW build (0 bytes for
        // some minimal exports, 12 bytes for non-instanced PC, 16 bytes for
        // pre-instanced PC, possibly larger for PS2/Xbox splits). Derive its
        // size from the extension's chunk header so the cursor stays aligned
        // for chunks that follow.
        let parsed = 4usize
            + used_bone_count as usize
            + (vertices_count as usize) * 4
            + (vertices_count as usize) * 16
            + (bone_count as usize) * 64;
        let chunk_len = header.length as usize;
        if parsed > chunk_len {
            anyhow::bail!(
                "SkinPlugin: parsed {} bytes exceeds chunk length {}",
                parsed,
                chunk_len
            );
        }
        let tail_len = chunk_len - parsed;
        let unknown2 = cursor.read_u8_vec(tail_len)?;

        Ok(Self {
            used_bones,
            bone_indices,
            weights,
            matrix,
            unknown2,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct NodeNamePlugin {
    name: String,
}

impl NodeNamePlugin {
    pub fn read(cursor: &mut dyn Read, header: ChunkHeader) -> anyhow::Result<Self> {
        let name = cursor.read_u8_vec(header.length as usize)?;
        let name = encoding::all::UTF_8
            .decode(&name, DecoderTrap::Ignore)
            .or(Ok::<String, ()>("DEC_ERR".to_string()))
            .unwrap();
        Ok(Self { name })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(name: String) -> Self {
        Self { name }
    }
}

#[derive(Debug, Serialize)]
pub enum UserData {
    Integer(i32),
    Float(f32),
    String(String),
}

impl UserData {
    pub fn get_string(&self) -> Option<String> {
        if let Self::String(s) = self {
            Some(s.clone())
        } else {
            None
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UserDataPlugin {
    data: HashMap<String, Vec<UserData>>,
}

impl UserDataPlugin {
    pub fn read(cursor: &mut dyn Read, _: ChunkHeader) -> anyhow::Result<Self> {
        let entry_count = cursor.read_u32_le()?;
        let mut data = HashMap::new();
        for _ in 0..entry_count {
            let entry_name = Self::read_string(cursor)?;
            let ty = cursor.read_u32_le()?;
            let item_count = cursor.read_u32_le()?;

            let mut items = vec![];
            for _ in 0..item_count {
                match ty {
                    1 => items.push(UserData::Integer(cursor.read_i32::<LittleEndian>()?)),
                    2 => items.push(UserData::Float(cursor.read_f32::<LittleEndian>()?)),
                    3 => items.push(UserData::String(Self::read_string(cursor)?)),
                    _ => panic!("Not supported"),
                }
            }

            data.insert(entry_name, items);
        }

        Ok(Self { data })
    }

    pub fn data(&self) -> &HashMap<String, Vec<UserData>> {
        &self.data
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(data: HashMap<String, Vec<UserData>>) -> Self {
        Self { data }
    }

    fn read_string(cursor: &mut dyn Read) -> anyhow::Result<String> {
        let length = cursor.read_u32_le()?;
        cursor.read_string(length as usize)
    }
}

#[derive(Debug, Serialize)]
pub struct BinMesh {
    pub material: u32,
    pub indices: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct BinMeshPlugin {
    flag: u32,
    total_indices_count: u32,
    meshes: Vec<BinMesh>,
}

impl BinMeshPlugin {
    pub fn read(cursor: &mut dyn Read, _: ChunkHeader) -> anyhow::Result<Self> {
        let flag = cursor.read_u32_le()?;
        let mesh_count = cursor.read_u32_le()?;
        let total_indices_count = cursor.read_u32_le()?;

        let mut meshes = vec![];
        for _ in 0..mesh_count {
            let indices_count = cursor.read_u32_le()?;
            let material = cursor.read_u32_le()?;

            meshes.push(BinMesh {
                material,
                indices: cursor.read_dw_vec(indices_count as usize)?,
            });
        }

        Ok(Self {
            flag,
            total_indices_count,
            meshes,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct UnknownPlugin {
    pub ty: ChunkType,
    pub data: Vec<u8>,
}

impl UnknownPlugin {
    pub fn read(cursor: &mut dyn Read, header: ChunkHeader) -> anyhow::Result<Self> {
        let data = cursor.read_u8_vec(header.length as usize)?;
        Ok(Self {
            ty: header.ty,
            data,
        })
    }
}

/// PAL4's per-material lightmap plugin (RW chunk type `0x120`).
///
/// Body layout (observed but **not standardized** across PAL4 BSPs):
///
/// ```text
/// u8[N]             preamble  (variable length; the synthetic test
///                              fixture uses [4,4,1,3,1,6] u32s but
///                              real PAL4 BSPs drift)
/// ChunkHeader       TEXTURE chunk header (ty = 0x06)
/// Texture body      STRUCT(modes) + STRING(name) + STRING(mask) +
///                   EXTENSION(empty)
/// ```
///
/// Because the preamble length / contents vary across exporter
/// versions, we don't try to decode it structurally. Instead we read
/// the whole chunk body into a buffer, scan it for a well-formed
/// `TEXTURE` chunk header (`06 00 00 00`, with a length that fits
/// inside the remaining buffer and an immediately-following `STRUCT`
/// header), and parse the nested chunk via the standard `Texture::read`
/// path. The opaque preamble bytes are preserved verbatim in
/// `raw_preamble` for diagnostic round-tripping.
///
/// If no TEXTURE chunk is found (defensive — every shipped material
/// we know of has one), `texture` is `None`. We never return `Err`
/// for a parseable 0x120 chunk: missing/garbled bodies would otherwise
/// take down the BSP loader for the whole scene.
///
/// Pair `texture.name` (e.g. `"Cylinder1746LightingMap"`) with the
/// material's primary `Material::texture.name` (the diffuse, e.g.
/// `"fz01"`) to sample the baked lightmap with the secondary UV set.
///
/// Note: PAL4 DFFs also occasionally carry a 0x120 chunk, but only at
/// the *atomic* level (sky/water/trans scene blocks) and with a
/// degenerate body of just `[0x01, 0, 0, 0]` and no embedded TEXTURE
/// — i.e. a per-atomic flag, not a material lightmap atlas reference.
/// Material-level lightmap plugins (the kind this struct primarily
/// models) appear only on BSP materials in shipped PAL4 data; no DFF
/// material has been observed with one.
#[derive(Debug, Serialize)]
pub struct LightMapPlugin {
    pub raw_preamble: Vec<u8>,
    pub texture: Option<Texture>,
}

impl LightMapPlugin {
    pub fn read(cursor: &mut dyn Read, header: ChunkHeader) -> anyhow::Result<Self> {
        let body = cursor.read_u8_vec(header.length as usize)?;
        let (raw_preamble, texture) = match Self::find_texture(&body) {
            Some((offset, tex)) => (body[..offset].to_vec(), Some(tex)),
            None => (body, None),
        };
        Ok(Self {
            raw_preamble,
            texture,
        })
    }

    /// Scan `data` for the first well-formed nested RW `TEXTURE` chunk
    /// and parse it via `Texture::read`. Returns the byte offset of
    /// the chunk header (so the caller can preserve the preamble) and
    /// the parsed `Texture` on success.
    fn find_texture(data: &[u8]) -> Option<(usize, Texture)> {
        use std::io::Cursor;
        let n = data.len();
        let mut i = 0usize;
        while i + 12 <= n {
            // TEXTURE chunk-header signature `[0x06, 0, 0, 0]` at u32
            // alignment. PAL4 preambles are u32-aligned in every BSP
            // we've seen, so we step in 4-byte increments.
            if data[i] == 0x06 && data[i + 1] == 0 && data[i + 2] == 0 && data[i + 3] == 0 {
                let len = u32::from_le_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]])
                    as usize;
                let body_start = i + 12;
                if body_start + len <= n && len >= 12 {
                    // Validate the immediately-following STRUCT
                    // header before committing to a full Texture::read
                    // — minor preamble drift can produce a false
                    // `06 00 00 00` match.
                    if data[body_start] == 0x01
                        && data[body_start + 1] == 0
                        && data[body_start + 2] == 0
                        && data[body_start + 3] == 0
                    {
                        let mut cur = Cursor::new(&data[body_start..body_start + len]);
                        if let Ok(tex) = Texture::read(&mut cur) {
                            return Some((i, tex));
                        }
                    }
                }
            }
            i += 4;
        }
        None
    }
}
