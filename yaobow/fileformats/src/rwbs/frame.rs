use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::Vec3f;

use super::{extension::Extension, plugins::hanim::HAnimPlugin};

#[derive(Debug, Serialize)]
pub struct Frame {
    pub right: Vec3f,
    pub up: Vec3f,
    pub at: Vec3f,
    pub pos: Vec3f,
    pub parent: i32,
    pub unknown: u32,

    pub extensions: Vec<Extension>,
}

impl Frame {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let right = Self::read_vec3(cursor)?;
        let up = Self::read_vec3(cursor)?;
        let at = Self::read_vec3(cursor)?;
        let pos = Self::read_vec3(cursor)?;
        let parent = cursor.read_i32::<LittleEndian>()?;
        let unknown = cursor.read_u32_le()?;

        Ok(Self {
            right,
            up,
            at,
            pos,
            parent,
            unknown,
            extensions: vec![],
        })
    }

    pub fn set_extensions(&mut self, ext: Vec<Extension>) {
        self.extensions = ext;
    }

    pub fn extensions(&self) -> &[Extension] {
        &self.extensions
    }

    pub fn hanim_plugin(&self) -> Option<&HAnimPlugin> {
        self.extensions
            .iter()
            .flat_map(|e| {
                if let Extension::HAnimPlugin(hanim) = e {
                    Some(hanim)
                } else {
                    None
                }
            })
            .next()
    }

    pub fn name(&self) -> Option<String> {
        for e in &self.extensions {
            if let Extension::UserDataPlugin(u) = e {
                if let Some(names) = u.data().get("name") {
                    return names.get(0).and_then(|s| s.get_string());
                }
            }
        }

        None
    }

    fn read_vec3(cursor: &mut dyn Read) -> anyhow::Result<Vec3f> {
        let x = cursor.read_f32::<LittleEndian>()?;
        let y = cursor.read_f32::<LittleEndian>()?;
        let z = cursor.read_f32::<LittleEndian>()?;
        Ok(Vec3f { x, y, z })
    }
}
