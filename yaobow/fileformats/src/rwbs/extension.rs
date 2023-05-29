use std::{collections::HashMap, io::Read};

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use encoding::{types::Encoding, DecoderTrap};
use serde::Serialize;

use crate::rwbs::{check_ty, ChunkHeader, ChunkType};

use super::{plugins::hanim::HAnimPlugin, Matrix44f};

#[derive(Debug, Serialize)]
pub enum Extension {
    HAnimPlugin(HAnimPlugin),
    SkinPlugin(SkinPlugin),
    UserDataPlugin(UserDataPlugin),
    NodeNamePlugin(NodeNamePlugin),
    BinMeshPlugin(BinMeshPlugin),
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
                ChunkType::PLUGIN_HANIM => {
                    Extension::HAnimPlugin(HAnimPlugin::read(cursor, ext_header)?)
                }

                ChunkType::PLUGIN_SKIN => {
                    Extension::SkinPlugin(SkinPlugin::read(cursor, vertices_count)?)
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
pub struct SkinPlugin {
    pub used_bones: Vec<u8>,
    pub bone_indices: Vec<[u8; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub matrix: Vec<Matrix44f>,
    unknown2: Vec<u8>,
}

impl SkinPlugin {
    pub fn read(cursor: &mut dyn Read, vertices_count: u32) -> anyhow::Result<Self> {
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

        let unknown2 = cursor.read_u8_vec(12)?;

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
    ty: ChunkType,
    unknown: Vec<u8>,
}

impl UnknownPlugin {
    pub fn read(cursor: &mut dyn Read, header: ChunkHeader) -> anyhow::Result<Self> {
        let unknown = cursor.read_u8_vec(header.length as usize)?;
        Ok(Self {
            ty: header.ty,
            unknown,
        })
    }
}
