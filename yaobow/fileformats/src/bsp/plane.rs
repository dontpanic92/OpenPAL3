use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::{check_ty, ChunkHeader, ChunkType, FormatFlag};

use super::world::Sector;

#[derive(Debug, Serialize)]
pub struct Plane {
    pub sector_type: u32,
    pub unknown: u32,
    pub value: f32,
    pub unknown2: f32,
    pub unknown3: u32,
    pub left_is_world_sector: u32,
    pub right_is_world_sector: u32,
    pub left_value: f32,
    pub right_value: f32,
}

impl Plane {
    pub fn read_header(cursor: &mut dyn Read) -> anyhow::Result<ChunkHeader> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::PLANE_SECTOR);

        Ok(header)
    }

    pub fn read(cursor: &mut dyn Read, flag: FormatFlag) -> anyhow::Result<Self> {
        println!("plane reading start");
        let sector_type = cursor.read_u32_le()?;
        let unknown = cursor.read_u32_le()?;
        let unknown2 = cursor.read_f32::<LittleEndian>()?;
        let unknown3 = cursor.read_u32_le()?;
        let value = cursor.read_f32::<LittleEndian>()?;
        let left_is_world_sector = cursor.read_u32_le()?;
        let right_is_world_sector = cursor.read_u32_le()?;
        let left_value = cursor.read_f32::<LittleEndian>()?;
        let right_value = cursor.read_f32::<LittleEndian>()?;
        println!(
            "in plane: {} {} {} {} {} {}",
            sector_type, unknown, unknown2, unknown3, left_is_world_sector, right_is_world_sector
        );

        if left_is_world_sector != 0 {
            let _ = Sector::read_header(cursor)?;
            let sector = Sector::read(cursor, flag)?;
        } else {
            let _ = Plane::read_header(cursor)?;
            let plane = Plane::read(cursor, flag)?;
        }

        if right_is_world_sector != 0 {
            let _ = Sector::read_header(cursor)?;
            let sector = Sector::read(cursor, flag)?;
        } else {
            let _ = Plane::read_header(cursor)?;
            let plane = Plane::read(cursor, flag)?;
        }

        println!("plane reading complete");

        Ok(Self {
            sector_type,
            unknown,
            value,
            unknown2,
            unknown3,
            left_is_world_sector,
            right_is_world_sector,
            left_value,
            right_value,
        })
    }
}
