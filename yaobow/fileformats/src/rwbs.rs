use std::io::{Cursor, Read};

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

/**
 * RWBS formats details:
 *      https://helco.github.io/zzdocs/resources/
 *      https://rwsreader.sourceforge.net/
 */

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ChunkType(pub u32);

impl ChunkType {
    pub const STRUCT: Self = Self(0x1);
    pub const STRING: Self = Self(0x2);
    pub const EXTENSION: Self = Self(0x3);
    pub const TEXTURE: Self = Self(0x6);
    pub const MATERIAL: Self = Self(0x7);
    pub const MATERIAL_LIST: Self = Self(0x8);
    pub const ATOMIC_SECTOR: Self = Self(0x9);
    pub const PLANE_SECTOR: Self = Self(0xa);
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

#[derive(Copy, Clone, Debug, Serialize)]
pub struct FormatFlag(pub u32);
impl FormatFlag {
    pub const TRISTRIP: Self = Self(0x1);
    pub const POSITIONS: Self = Self(0x2);
    pub const TEXTURED: Self = Self(0x4);
    pub const PRELIT: Self = Self(0x8);
    pub const NORMALS: Self = Self(0x10);
    pub const LIGHT: Self = Self(0x20);
    pub const MODULATE_MATERIAL_COLOR: Self = Self(0x40);
    pub const TEXTURED2: Self = Self(0x80);
    pub const NATIVE: Self = Self(0x01000000);
    pub const NATIVE_INSTANCE: Self = Self(0x02000000);
    pub const SECTORS_OVERLAP: Self = Self(0x40000000);

    pub fn contains(&self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }
}

#[derive(Debug, Serialize)]
pub struct Vec3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3f {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let x = cursor.read_f32::<LittleEndian>()?;
        let y = cursor.read_f32::<LittleEndian>()?;
        let z = cursor.read_f32::<LittleEndian>()?;
        Ok(Self { x, y, z })
    }
}

#[derive(Debug, Serialize)]
pub struct PrelitColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl PrelitColor {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let r = cursor.read_u8()?;
        let g = cursor.read_u8()?;
        let b = cursor.read_u8()?;
        let a = cursor.read_u8()?;
        Ok(Self { r, g, b, a })
    }
}
#[derive(Debug, Serialize)]
pub struct Normal {
    pub x: u8,
    pub y: u8,
    pub z: u8,
    pub p: u8,
}

impl Normal {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let x = cursor.read_u8()?;
        let y = cursor.read_u8()?;
        let z = cursor.read_u8()?;
        let p = cursor.read_u8()?;
        Ok(Self { x, y, z, p })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TexCoord {
    pub u: f32,
    pub v: f32,
}

impl TexCoord {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let u = cursor.read_f32::<LittleEndian>()?;
        let v = cursor.read_f32::<LittleEndian>()?;
        Ok(Self { u, v })
    }
}

#[derive(Debug, Serialize)]
pub struct Triangle {
    pub index: [u16; 3],
    pub material: u16,
}

#[derive(Debug, Serialize)]
pub struct ChunkHeader {
    pub ty: ChunkType,
    pub length: u32,
    pub build_number: u16,
    pub version: u16,
}

impl ChunkHeader {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
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

#[derive(thiserror::Error, Debug)]
pub enum RwbsReadError {
    #[error("Incorrect chunk type, expected {1}, found {0}")]
    IncorrectChunkFormat(u32, u32),
}

macro_rules! check_ty {
    ($actual: expr, $expected: expr) => {
        if $expected != $actual {
            return Err(crate::rwbs::RwbsReadError::IncorrectChunkFormat(
                $actual.0,
                $expected.0,
            ))?;
        }
    };
}

pub(crate) use check_ty;

pub fn list_chunks(data: &[u8]) -> anyhow::Result<Vec<ChunkHeader>> {
    let mut cursor = Cursor::new(data);
    let mut chunks = vec![];

    while !cursor.is_empty() {
        let chunk = ChunkHeader::read(&mut cursor)?;
        cursor.set_position(cursor.position() + chunk.length as u64);
        chunks.push(chunk);
    }

    Ok(chunks)
}
