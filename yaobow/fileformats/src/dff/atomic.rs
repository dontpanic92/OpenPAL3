use std::io::Read;

use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::{check_ty, ChunkHeader, ChunkType, RwbsReadError};

#[derive(Debug, Serialize)]
pub struct Atomic {
    pub frame: u32,
    pub geometry: u32,
    pub unknown: u32,
    pub unknown2: u32,
    pub extension: Vec<u8>,
}

impl Atomic {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRUCT);

        let frame = cursor.read_u32_le()?;
        let geometry = cursor.read_u32_le()?;
        let unknown = cursor.read_u32_le()?;
        let unknown2 = cursor.read_u32_le()?;

        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::EXTENSION);

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
