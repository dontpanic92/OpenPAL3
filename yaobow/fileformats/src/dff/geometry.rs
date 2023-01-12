use std::{error::Error, io::Read};

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use super::{material::Material, ChunkHeader, ChunkType, DffReadError, Vec3f};

#[derive(Debug, Serialize)]
pub struct GeometryFlags(pub u32);
impl GeometryFlags {
    pub const TRISTRIP: Self = Self(0x1);
    pub const POSITIONS: Self = Self(0x2);
    pub const TEXTURED: Self = Self(0x4);
    pub const PRELIT: Self = Self(0x8);
    pub const NORMALS: Self = Self(0x10);
    pub const LIGHT: Self = Self(0x20);
    pub const MODULATE_MATERIAL_COLOR: Self = Self(0x40);
    pub const TEXTURED2: Self = Self(0x80);
    pub const NATIVE: Self = Self(0x01);
    pub const NATIVE_INSTANCE: Self = Self(0x02);
    pub const FLAGS_MASK: Self = Self(0xFF);
    pub const NATIVE_FLAGS_MASK: Self = Self(0x0F);

    pub fn contains(&self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }
}

#[derive(Debug, Serialize)]
pub struct TexCoord {
    pub u: f32,
    pub v: f32,
}

#[derive(Debug, Serialize)]
pub struct Triangle {
    pub index: [u16; 3],
    pub material: u16,
}

#[derive(Debug, Serialize)]
pub struct GeometryMorphTarget {
    pub bounding_sphere_x: f32,
    pub bounding_sphere_y: f32,
    pub bounding_sphere_z: f32,
    pub radius: f32,

    pub vertices: Option<Vec<Vec3f>>,
    pub normals: Option<Vec<Vec3f>>,
}

impl GeometryMorphTarget {
    pub fn read(cursor: &mut dyn Read, vertex_count: u32) -> Result<Self, Box<dyn Error>> {
        let bounding_sphere_x = cursor.read_f32::<LittleEndian>()?;
        let bounding_sphere_y = cursor.read_f32::<LittleEndian>()?;
        let bounding_sphere_z = cursor.read_f32::<LittleEndian>()?;
        let radius = cursor.read_f32::<LittleEndian>()?;

        let has_positions = cursor.read_u32_le()?;
        let has_normals = cursor.read_u32_le()?;

        let mut vertices = None;
        let mut normals = None;

        if has_positions != 0 {
            let mut vertex_vec = vec![];
            for _ in 0..vertex_count {
                let vertex = Vec3f::read(cursor)?;
                vertex_vec.push(vertex);
            }

            vertices = Some(vertex_vec);
        }

        if has_normals != 0 {
            let mut normal_vec = vec![];
            for _ in 0..vertex_count {
                let normal = Vec3f::read(cursor)?;
                normal_vec.push(normal);
            }

            normals = Some(normal_vec);
        }

        Ok(Self {
            bounding_sphere_x,
            bounding_sphere_y,
            bounding_sphere_z,
            radius,
            vertices,
            normals,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct Geometry {
    pub flags: GeometryFlags,
    pub vertices_count: u32,
    pub prelit: Option<Vec<u32>>,
    pub texcoord_sets: Option<Vec<Vec<TexCoord>>>,
    pub triangles: Vec<Triangle>,
    pub materials: Vec<Material>,
    pub morph_targets: Vec<GeometryMorphTarget>,
}

impl Geometry {
    pub fn read(cursor: &mut dyn Read) -> Result<Self, Box<dyn Error>> {
        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::STRUCT {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let flags = GeometryFlags(cursor.read_u32_le()?);
        let triangle_count = cursor.read_u32_le()?;
        let vertices_count = cursor.read_u32_le()?;
        let morph_target_count = cursor.read_u32_le()?;
        let mut prelit = None;

        if flags.contains(GeometryFlags::PRELIT) {
            prelit = Some(cursor.read_dw_vec(vertices_count as usize)?);
        }

        let texcoord_set_count = (flags.0 & 0x00ff0000) >> 16;
        let mut texcoord_sets = None;
        if flags.contains(GeometryFlags::TEXTURED) || flags.contains(GeometryFlags::TEXTURED2) {
            let mut texcoord_sets_vec = vec![];
            for _ in 0..texcoord_set_count {
                let mut tex_coord_vec = vec![];
                for _ in 0..vertices_count {
                    let u = cursor.read_f32::<LittleEndian>()?;
                    let v = cursor.read_f32::<LittleEndian>()?;
                    tex_coord_vec.push(TexCoord { u, v });
                }
                texcoord_sets_vec.push(tex_coord_vec);
            }

            texcoord_sets = Some(texcoord_sets_vec);
        }

        let mut triangles = vec![];
        for _ in 0..triangle_count {
            let i1 = cursor.read_u16_le()?;
            let i0 = cursor.read_u16_le()?;
            let material = cursor.read_u16_le()?;
            let i2 = cursor.read_u16_le()?;
            triangles.push(Triangle {
                index: [i0, i1, i2],
                material,
            });
        }

        let mut morph_targets = vec![];
        for _ in 0..morph_target_count {
            let morph_target = GeometryMorphTarget::read(cursor, vertices_count)?;
            morph_targets.push(morph_target);
        }

        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::MATERIAL_LIST {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let materials = Self::read_material_list(cursor)?;

        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::EXTENSION {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let mut len = 0;
        while len < header.length {
            let header = ChunkHeader::read(cursor)?;
            len += 12 + header.length;
            cursor.skip(header.length as usize)?;
        }

        Ok(Self {
            flags,
            vertices_count,
            prelit,
            texcoord_sets,
            triangles,
            materials,
            morph_targets,
        })
    }

    fn read_material_list(cursor: &mut dyn Read) -> Result<Vec<Material>, Box<dyn Error>> {
        let header = ChunkHeader::read(cursor)?;
        if header.ty != ChunkType::STRUCT {
            return Err(DffReadError::IncorrectClumpFormat)?;
        }

        let mut material_vec = vec![];

        let material_count = cursor.read_u32_le()?;
        if material_count > 0 {
            let _unknown = cursor.read_dw_vec(material_count as usize)?;
            for _ in 0..material_count {
                let header = ChunkHeader::read(cursor)?;
                if header.ty != ChunkType::MATERIAL {
                    return Err(DffReadError::IncorrectClumpFormat)?;
                }

                material_vec.push(Material::read(cursor)?);
            }
        }

        Ok(material_vec)
    }
}
