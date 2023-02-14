use std::io::Read;

use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::{check_ty, ChunkHeader, ChunkType};

use super::Matrix44f;

#[derive(Debug, Serialize)]
pub enum Extension {
    SkinPlugin(SkinPlugin),
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
            let ext = match ext_header.ty {
                ChunkType::PLUGIN_SKIN => {
                    Extension::SkinPlugin(SkinPlugin::read(cursor, vertices_count)?)
                }
                ty => Extension::UnknownPlugin(UnknownPlugin::read(cursor, ty, ext_header.length)?),
            };

            extensions.push(ext);
            chunk_len -= 12 + ext_header.length;
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
    used_bones: Vec<u8>,
    bone_indices: Vec<[u8; 4]>,
    weights: Vec<[f32; 4]>,
    matrix: Vec<Matrix44f>,
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
pub struct UnknownPlugin {
    ty: ChunkType,
    unknown: Vec<u8>,
}

impl UnknownPlugin {
    pub fn read(cursor: &mut dyn Read, ty: ChunkType, len: u32) -> anyhow::Result<Self> {
        let unknown = cursor.read_u8_vec(len as usize)?;
        Ok(Self { ty, unknown })
    }
}
