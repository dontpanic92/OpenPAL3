use std::{error::Error, io::Read};

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use super::{ChunkHeader, ChunkType, DffReadError};

#[derive(Debug, Serialize)]
pub struct Texture {
    pub filter_mode: u32,
    pub address_mode_u: u32,
    pub address_mode_v: u32,
    pub name: String,
    pub mask_name: String,
}

impl Texture {
    pub fn read(cursor: &mut dyn Read) -> Result<Self, Box<dyn Error>> {
        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::STRUCT {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let modes = cursor.read_u32_le()?;

        let filter_mode = modes & _private::TEXTURE_FILTER_MODE_MASK;
        let address_mode_u = (modes & _private::TEXTURE_ADDRESS_MODE_U_MASK) >> 8;
        let address_mode_v = (modes & _private::TEXTURE_ADDRESS_MODE_V_MASK) >> 12;

        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::STRING {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let name = cursor.read_string(header.length as usize)?;

        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::STRING {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let mask_name = cursor.read_string(header.length as usize)?;

        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::EXTENSION {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

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
    pub fn read(cursor: &mut dyn Read) -> Result<Self, Box<dyn Error>> {
        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::STRUCT {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let unknown = cursor.read_u32_le()?;
        let color = cursor.read_u32_le()?;
        let unknown2 = cursor.read_u32_le()?;
        let textured = cursor.read_u32_le()?;
        let ambient = cursor.read_f32::<LittleEndian>()?;
        let specular = cursor.read_f32::<LittleEndian>()?;
        let diffuse = cursor.read_f32::<LittleEndian>()?;
        let mut texture = None;

        if textured != 0 {
            let header = ChunkHeader::read(cursor)?;
            texture = Some(Texture::read(cursor)?);
        }

        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::EXTENSION {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

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

mod _private {
    pub const TEXTURE_FILTER_MODE_MASK: u32 = 0x000000ff;
    pub const TEXTURE_ADDRESS_MODE_U_MASK: u32 = 0x00000f00;
    pub const TEXTURE_ADDRESS_MODE_V_MASK: u32 = 0x0000f000;
}
