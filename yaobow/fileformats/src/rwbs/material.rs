use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::{check_ty, ChunkHeader, ChunkType};

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
pub struct Material {
    pub unknown: u32,
    pub color: u32,
    pub unknown2: u32,
    pub texture: Option<Texture>,
    pub ambient: f32,
    pub specular: f32,
    pub diffuse: f32,
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

        cursor.skip(header.length as usize)?;

        Ok(Self {
            unknown,
            color,
            unknown2,
            texture,
            ambient,
            specular,
            diffuse,
        })
    }
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
        let _unknown = cursor.read_dw_vec(material_count as usize)?;
        for _ in 0..material_count {
            let header = ChunkHeader::read(cursor)?;
            check_ty!(header.ty, ChunkType::MATERIAL);

            material_vec.push(Material::read(cursor)?);
        }
    }

    Ok(material_vec)
}

mod _private {
    pub const TEXTURE_FILTER_MODE_MASK: u32 = 0x000000ff;
    pub const TEXTURE_ADDRESS_MODE_U_MASK: u32 = 0x00000f00;
    pub const TEXTURE_ADDRESS_MODE_V_MASK: u32 = 0x0000f000;
}
