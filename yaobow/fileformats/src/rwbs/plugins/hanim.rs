use std::io::{Cursor, Read};

use binrw::{binrw, BinRead};
use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::ChunkHeader;

#[binrw]
#[brw(little)]
#[derive(Debug, Serialize)]
pub struct HAnimPlugin {
    pub header: HAnimHeader,

    #[br(if(header.bone_count > 0))]
    pub unknown: Option<HAnimUnknown>,

    #[br(count = header.bone_count)]
    pub block_types: Vec<HAnimBone>,
}

impl HAnimPlugin {
    pub fn read(cursor: &mut dyn Read, header: ChunkHeader) -> anyhow::Result<Self> {
        let buf = cursor.read_u8_vec(header.length as usize)?;
        Ok(<Self as BinRead>::read(&mut Cursor::new(buf))?)
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug, Serialize)]
pub struct HAnimHeader {
    pub version: u32,
    pub id: u32,
    pub bone_count: u32,
}

#[binrw]
#[brw(little)]
#[derive(Debug, Serialize)]
pub struct HAnimUnknown {
    unknown1: u32,
    unknown2: u32,
}

#[binrw]
#[brw(little)]
#[derive(Debug, Serialize)]
pub struct HAnimBone {
    pub id: u32,
    pub index: u32,
    pub ty: u32,
}
