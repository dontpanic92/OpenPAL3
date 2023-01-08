use std::{error::Error, io::Read};

use common::read_ext::ReadExt;
use serde::Serialize;

use crate::dff::DffReadError;

use super::{ChunkHeader, ChunkType};

#[derive(Debug, Serialize)]
pub struct Atomic {
    frame: u32,
    geometry: u32,
    unknown: u32,
    unknown2: u32,
    extension: Vec<u8>,
}

impl Atomic {
    pub fn read(cursor: &mut dyn Read) -> Result<Self, Box<dyn Error>> {
        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::STRUCT {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let frame = cursor.read_u32_le()?;
        let geometry = cursor.read_u32_le()?;
        let unknown = cursor.read_u32_le()?;
        let unknown2 = cursor.read_u32_le()?;

        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::EXTENSION {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let mut extension = vec![0u8; header.length as usize];
        cursor.read_exact(&mut extension)?;

        Ok(Self {
            frame,
            geometry,
            unknown,
            unknown2,
            extension,
        })
    }
}
