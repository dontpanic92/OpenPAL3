use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::{check_ty, ChunkHeader, ChunkType, FormatFlag, TexCoord, Triangle, Vec3f};

use super::material::Material;

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
    pub fn read(cursor: &mut dyn Read, vertex_count: u32) -> anyhow::Result<Self> {
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
    pub flags: FormatFlag,
    pub vertices_count: u32,
    pub prelit: Option<Vec<u32>>,
    pub texcoord_sets: Vec<Vec<TexCoord>>,
    pub triangles: Vec<Triangle>,
    pub materials: Vec<Material>,
    pub morph_targets: Vec<GeometryMorphTarget>,
}

impl Geometry {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRUCT);

        let flags = FormatFlag(cursor.read_u32_le()?);
        let triangle_count = cursor.read_u32_le()?;
        let vertices_count = cursor.read_u32_le()?;
        let morph_target_count = cursor.read_u32_le()?;
        let mut prelit = None;

        if flags.contains(FormatFlag::PRELIT) {
            prelit = Some(cursor.read_dw_vec(vertices_count as usize)?);
        }

        let texcoord_set_count = (flags.0 & 0x00ff0000) >> 16;
        let mut texcoord_sets = vec![];
        if flags.contains(FormatFlag::TEXTURED) || flags.contains(FormatFlag::TEXTURED2) {
            for _ in 0..texcoord_set_count {
                let mut tex_coord_vec = vec![];
                for _ in 0..vertices_count {
                    tex_coord_vec.push(TexCoord::read(cursor)?);
                }

                texcoord_sets.push(tex_coord_vec);
            }
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

        let _ = super::material::read_material_list_header(cursor)?;
        let materials = super::material::read_material_list(cursor)?;

        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::EXTENSION);

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
}
