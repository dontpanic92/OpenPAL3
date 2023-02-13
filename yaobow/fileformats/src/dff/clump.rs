use std::io::Read;

use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::{check_ty, ChunkHeader, ChunkType};

use super::{atomic::Atomic, frame::Frame, geometry::Geometry};

#[derive(Debug, Serialize)]
pub struct Clump {
    pub header: ChunkHeader,
    pub light_count: u32,
    pub camera_count: u32,
    pub frames: Vec<Frame>,
    pub geometries: Vec<Geometry>,
    pub atomics: Vec<Atomic>,
    pub extension: Vec<u8>,
}

impl Clump {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRUCT);

        let atomic_count = cursor.read_u32_le()?;
        let light_count = cursor.read_u32_le()?;
        let camera_count = cursor.read_u32_le()?;

        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::FRAME_LIST);

        let frames = Self::read_frame_list(cursor)?;

        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::GEOMETRY_LIST);

        let geometries = Self::read_geometry_list(cursor)?;

        let mut atomics = vec![];
        for _ in 0..atomic_count {
            let header = ChunkHeader::read(cursor)?;
            check_ty!(header.ty, ChunkType::ATOMIC);

            let atomic = Atomic::read(cursor)?;
            atomics.push(atomic);
        }

        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::EXTENSION);

        let mut extension = vec![0u8; header.length as usize];
        cursor.read_exact(&mut extension)?;

        Ok(Self {
            header,
            light_count,
            camera_count,
            frames,
            geometries,
            atomics,
            extension,
        })
    }

    fn read_frame_list(cursor: &mut dyn Read) -> anyhow::Result<Vec<Frame>> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRUCT);

        let frame_count = cursor.read_u32_le()?;
        let mut frames = vec![];

        for _ in 0..frame_count {
            let frame = Frame::read(cursor)?;
            frames.push(frame);
        }

        for _ in 0..frame_count {
            let header = ChunkHeader::read(cursor)?;
            check_ty!(header.ty, ChunkType::EXTENSION);

            cursor.skip(header.length as usize)?;
        }

        Ok(frames)
    }

    fn read_geometry_list(cursor: &mut dyn Read) -> anyhow::Result<Vec<Geometry>> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRUCT);

        let count = cursor.read_u32_le()?;
        let mut geometries = vec![];
        for _ in 0..count {
            let header = ChunkHeader::read(cursor)?;
            check_ty!(header.ty, ChunkType::GEOMETRY);

            let geometry = Geometry::read(cursor)?;
            geometries.push(geometry);
        }

        Ok(geometries)
    }
}
