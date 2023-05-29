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
    version: u32,
    id: u32,
    bone_count: u32,
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
    id: u32,
    index: u32,
    ty: u32,
}
