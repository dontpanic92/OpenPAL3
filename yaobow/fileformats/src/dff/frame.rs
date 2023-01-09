use std::{error::Error, io::Read};

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use super::Vec3f;

#[derive(Debug, Serialize)]
pub struct Frame {
    pub right: Vec3f,
    pub up: Vec3f,
    pub at: Vec3f,
    pub pos: Vec3f,
    pub parent: u32,
    pub unknown: u32,
}

impl Frame {
    pub fn read(cursor: &mut dyn Read) -> Result<Self, Box<dyn Error>> {
        let right = Self::read_vec3(cursor)?;
        let up = Self::read_vec3(cursor)?;
        let at = Self::read_vec3(cursor)?;
        let pos = Self::read_vec3(cursor)?;
        let parent = cursor.read_u32_le()?;
        let unknown = cursor.read_u32_le()?;

        Ok(Self {
            right,
            up,
            at,
            pos,
            parent,
            unknown,
        })
    }

    fn read_vec3(cursor: &mut dyn Read) -> Result<Vec3f, Box<dyn Error>> {
        let x = cursor.read_f32::<LittleEndian>()?;
        let y = cursor.read_f32::<LittleEndian>()?;
        let z = cursor.read_f32::<LittleEndian>()?;
        Ok(Vec3f { x, y, z })
    }
}
