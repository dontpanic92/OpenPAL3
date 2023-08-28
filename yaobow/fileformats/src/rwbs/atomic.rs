use std::io::Read;

use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::{check_ty, extension::Extension, ChunkHeader, ChunkType};

#[derive(Debug, Serialize)]
pub struct Atomic {
    pub frame: u32,
    pub geometry: u32,
    pub unknown: u32,
    pub unknown2: u32,
    pub extensions: Vec<Extension>,
}

impl Atomic {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRUCT);

        let frame = cursor.read_u32_le()?;
        let geometry = cursor.read_u32_le()?;
        let unknown = cursor.read_u32_le()?;
        let unknown2 = cursor.read_u32_le()?;

        let extensions = Extension::read(cursor, 0)?;

        Ok(Self {
            frame,
            geometry,
            unknown,
            unknown2,
            extensions,
        })
    }

    pub fn contains_right_to_render(&self) -> bool {
        self.extensions
            .iter()
            .any(|e| matches!(e, Extension::RightToRender(_)))
    }
}
