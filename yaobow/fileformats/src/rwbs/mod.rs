pub mod anm;
pub mod atomic;
pub mod clump;
pub mod extension;
pub mod frame;
pub mod geometry;
pub mod material;
pub mod plugins;
pub mod sector;
pub mod world;

use std::io::{Cursor, Read};

use binrw::{binrw, BinRead, BinResult};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use clump::Clump;
use common::read_ext::ReadExt;
use serde::Serialize;
use world::World;

/**
 * RWBS formats details:
 *      https://helco.github.io/zzdocs/resources/
 *      https://gtamods.com/wiki/
 *      https://rwsreader.sourceforge.net/
 */

#[binrw]
#[brw(little)]
#[derive(Debug, Serialize, PartialEq, Eq, Copy, Clone)]
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
    pub const ANIM_ANIMATION: Self = Self(0x1b);
    pub const CHUNK_GROUP_START: Self = Self(0x29);
    pub const CHUNK_GROUP_END: Self = Self(0x2a);

    pub const PLUGIN_MORPH: Self = Self(0x105);
    pub const PLUGIN_SKIN: Self = Self(0x116);
    pub const PLUGIN_HANIM: Self = Self(0x11e);
    pub const PLUGIN_USERDATA: Self = Self(0x11f);
    pub const PLUGIN_BINMESH: Self = Self(0x50e);

    pub const PLUGIN_NODENAME: Self = Self(0x0253f2fe);
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

#[binrw::parser(reader, endian)]
fn float_parser(half_float: bool) -> BinResult<f32> {
    if half_float {
        if endian == binrw::Endian::Little {
            Ok(half::f16::from_bits(reader.read_u16::<LittleEndian>()?).to_f32())
        } else {
            Ok(half::f16::from_bits(reader.read_u16::<BigEndian>()?).to_f32())
        }
    } else {
        if endian == binrw::Endian::Little {
            Ok(reader.read_f32::<LittleEndian>()?)
        } else {
            Ok(reader.read_f32::<BigEndian>()?)
        }
    }
}

#[binrw]
#[brw(little)]
#[brw(import{half_float: bool = false})]
#[derive(Debug, Serialize)]
pub struct Vec4f {
    #[br(parse_with = float_parser)]
    #[br(args(half_float))]
    pub x: f32,

    #[br(parse_with = float_parser)]
    #[br(args(half_float))]
    pub y: f32,

    #[br(parse_with = float_parser)]
    #[br(args(half_float))]
    pub z: f32,

    #[br(parse_with = float_parser)]
    #[br(args(half_float))]
    pub w: f32,
}

#[binrw]
#[brw(little)]
#[brw(import{half_float: bool = false})]
#[derive(Debug, Serialize, Clone)]
pub struct Vec3f {
    #[br(parse_with = float_parser)]
    #[br(args(half_float))]
    pub x: f32,

    #[br(parse_with = float_parser)]
    #[br(args(half_float))]
    pub y: f32,

    #[br(parse_with = float_parser)]
    #[br(args(half_float))]
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
pub struct Matrix44f(pub [f32; 16]);

impl Matrix44f {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let m = cursor.read_f32_vec(16)?;
        Ok(Self(m.try_into().unwrap()))
    }
}

#[binrw]
#[brw(little)]
#[brw(import{half_float: bool = false})]
#[derive(Debug, Serialize, Clone)]
pub struct Quaternion {
    #[br(parse_with = float_parser)]
    #[br(args(half_float))]
    pub x: f32,

    #[br(parse_with = float_parser)]
    #[br(args(half_float))]
    pub y: f32,

    #[br(parse_with = float_parser)]
    #[br(args(half_float))]
    pub z: f32,

    #[br(parse_with = float_parser)]
    #[br(args(half_float))]
    pub w: f32,
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

#[binrw]
#[brw(little)]
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

#[binrw]
#[brw(little)]
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

use self::anm::AnmAction;

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

pub fn read_dff(data: &[u8]) -> anyhow::Result<Vec<Clump>> {
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

pub fn read_bsp(data: &[u8]) -> anyhow::Result<Vec<World>> {
    let mut cursor = Cursor::new(data);
    let mut world = vec![];

    while !cursor.is_empty() {
        let chunk = ChunkHeader::read(&mut cursor)?;
        match chunk.ty {
            ChunkType::WORLD => world.push(World::read(&mut cursor)?),
            _ => cursor.set_position(cursor.position() + chunk.length as u64),
        }
    }

    Ok(world)
}

pub fn read_anm(data: &[u8]) -> anyhow::Result<Vec<AnmAction>> {
    let mut cursor = Cursor::new(data);
    let mut anim = vec![];

    while !cursor.is_empty() {
        let chunk = ChunkHeader::read(&mut cursor)?;
        match chunk.ty {
            ChunkType::ANIM_ANIMATION => anim.push(AnmAction::read(&mut cursor)?),
            _ => cursor.set_position(cursor.position() + chunk.length as u64),
        }
    }

    Ok(anim)
}
