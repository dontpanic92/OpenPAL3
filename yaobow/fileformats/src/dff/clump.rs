use std::{error::Error, io::Read};

use common::read_ext::ReadExt;
use serde::Serialize;

use super::{
    atomic::Atomic, frame::Frame, geometry::Geometry, ChunkHeader, ChunkType, DffReadError,
};

#[derive(Debug, Serialize)]
pub struct Clump {
    header: ChunkHeader,
    light_count: u32,
    camera_count: u32,
    frames: Vec<Frame>,
    geometries: Vec<Geometry>,
    atomics: Vec<Atomic>,
    extension: Vec<u8>,
}

impl Clump {
    pub fn read(cursor: &mut dyn Read) -> Result<Self, Box<dyn Error>> {
        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::STRUCT {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let atomic_count = cursor.read_u32_le()?;
        let light_count = cursor.read_u32_le()?;
        let camera_count = cursor.read_u32_le()?;

        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::FRAME_LIST {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let frames = Self::read_frame_list(cursor)?;

        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::GEOMETRY_LIST {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let geometries = Self::read_geometry_list(cursor)?;

        let mut atomics = vec![];
        for _ in 0..atomic_count {
            let header = ChunkHeader::read(cursor)?;
            if header.ty != ChunkType::ATOMIC {
                return Err(DffReadError::IncorrectClumpFormat)?;
            }

            let atomic = Atomic::read(cursor)?;
            atomics.push(atomic);
        }

        let header = ChunkHeader::read(cursor)?;

        if header.ty != ChunkType::EXTENSION {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

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

    fn read_frame_list(cursor: &mut dyn Read) -> Result<Vec<Frame>, Box<dyn Error>> {
        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::STRUCT {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let frame_count = cursor.read_u32_le()?;
        let mut frames = vec![];

        for _ in 0..frame_count {
            let frame = Frame::read(cursor)?;
            frames.push(frame);
        }

        for _ in 0..frame_count {
            let header = ChunkHeader::read(cursor)?;

            if header.ty != ChunkType::EXTENSION {
                return Err(DffReadError::IncorrectClumpFormat)?;
            }

            cursor.skip(header.length as usize)?;
        }

        Ok(frames)
    }

    fn read_geometry_list(cursor: &mut dyn Read) -> Result<Vec<Geometry>, Box<dyn Error>> {
        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::STRUCT {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let count = cursor.read_u32_le()?;
        let mut geometries = vec![];
        for _ in 0..count {
            let geo_header = ChunkHeader::read(cursor)?;
            if geo_header.ty != ChunkType::GEOMETRY {
                return Err(DffReadError::IncorrectClumpFormat)?;
            }

            let geometry = Geometry::read(cursor)?;
            geometries.push(geometry);
        }

        Ok(geometries)
    }
}
