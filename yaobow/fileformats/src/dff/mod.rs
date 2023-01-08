pub mod atomic;
pub mod clump;
pub mod frame;
pub mod geometry;
pub mod material;

use std::{
    error::Error,
    io::{Cursor, Read},
};

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use self::clump::Clump;

/**
 * DFF formats details: https://rwsreader.sourceforge.net/
 */

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ChunkType(u32);

impl ChunkType {
    pub const STRUCT: Self = Self(0x1);
    pub const STRING: Self = Self(0x2);
    pub const EXTENSION: Self = Self(0x3);
    pub const TEXTURE: Self = Self(0x6);
    pub const MATERIAL: Self = Self(0x7);
    pub const MATERIAL_LIST: Self = Self(0x8);
    pub const WORLD: Self = Self(0xb);
    pub const FRAME_LIST: Self = Self(0xe);
    pub const GEOMETRY: Self = Self(0xf);
    pub const CLUMP: Self = Self(0x10);
    pub const ATOMIC: Self = Self(0x14);
    pub const GEOMETRY_LIST: Self = Self(0x1a);
    pub const HANIM_ANIMATION: Self = Self(0x1b);
    pub const CHUNK_GROUP_START: Self = Self(0x29);
    pub const CHUNK_GROUP_END: Self = Self(0x2a);
}

#[derive(Debug, Serialize)]
pub struct Vec3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3f {
    pub fn read(cursor: &mut dyn Read) -> Result<Self, Box<dyn Error>> {
        let x = cursor.read_f32::<LittleEndian>()?;
        let y = cursor.read_f32::<LittleEndian>()?;
        let z = cursor.read_f32::<LittleEndian>()?;
        Ok(Self { x, y, z })
    }
}

#[derive(Debug)]
pub enum DffReadError {
    IncorrectClumpFormat,
}

impl Error for DffReadError {}

impl std::fmt::Display for DffReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Serialize)]
pub struct ChunkHeader {
    ty: ChunkType,
    length: u32,
    build_number: u16,
    version: u16,
}

impl ChunkHeader {
    pub fn read(cursor: &mut dyn Read) -> Result<Self, Box<dyn Error>> {
        let ty = cursor.read_u32_le()?;
        let length = cursor.read_u32_le()?;
        let build_number = cursor.read_u16_le()?;
        let version = cursor.read_u16_le()?;

        Ok(Self {
            ty: ChunkType(ty),
            length,
            build_number,
            version,
        })
    }
}

pub fn list_chunks(data: &[u8]) -> Result<Vec<ChunkHeader>, Box<dyn Error>> {
    let mut cursor = Cursor::new(data);
    let mut chunks = vec![];

    while !cursor.is_empty() {
        let chunk = ChunkHeader::read(&mut cursor)?;
        cursor.set_position(cursor.position() + chunk.length as u64);
        chunks.push(chunk);
    }

    Ok(chunks)
}

pub fn read_dff(data: &[u8]) -> Result<Vec<Clump>, Box<dyn Error>> {
    let mut cursor = Cursor::new(data);
    let mut chunks = vec![];

    while !cursor.is_empty() {
        let chunk = ChunkHeader::read(&mut cursor)?;
        match chunk.ty {
            ChunkType::CLUMP => chunks.push(Clump::read(&mut cursor)?),
            _ => cursor.set_position(cursor.position() + chunk.length as u64),
        }
    }

    Ok(chunks)
}
