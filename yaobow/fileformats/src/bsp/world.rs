use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use crate::{
    dff::material::Material,
    rwbs::{
        check_ty, ChunkHeader, ChunkType, FormatFlag, RwbsReadError, TexCoord, Triangle, Vec3f,
    },
};

#[derive(Debug, Serialize)]
pub struct World {
    pub header: ChunkHeader,
    root_is_world: u32,
    pub world_origin_x: f32,
    pub world_origin_y: f32,
    pub world_origin_z: f32,
    triangle_count: u32,
    vertices_count: u32,
    unknown: u32,
    count_sector: u32,
    unknown2: u32,
    flag: FormatFlag,
    pub bbox_max: Vec3f,
    pub bbox_min: Vec3f,

    pub materials: Vec<Material>,
    pub sectors: Vec<Sector>,
    pub extension: Vec<u8>,
}

impl World {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRUCT);

        let root_is_world = cursor.read_u32_le()?;
        let world_origin_x = -cursor.read_f32::<LittleEndian>()?;
        let world_origin_y = -cursor.read_f32::<LittleEndian>()?;
        let world_origin_z = -cursor.read_f32::<LittleEndian>()?;

        let triangle_count = cursor.read_u32_le()?;
        let vertices_count = cursor.read_u32_le()?;
        let unknown = cursor.read_u32_le()?;

        let count_sector = cursor.read_u32_le()?;
        let unknown2 = cursor.read_u32_le()?;
        let flag = FormatFlag(cursor.read_u32_le()?);
        let bbox_max = Vec3f::read(cursor)?;
        let bbox_min = Vec3f::read(cursor)?;

        let _ = crate::dff::material::read_material_list_header(cursor)?;
        println!("good9");
        let materials = crate::dff::material::read_material_list(cursor)?;

        println!("good10");
        let mut sectors = vec![];
        for _ in 0..count_sector {
            let _ = Sector::read_header(cursor)?;
            let sector = Sector::read(cursor, flag)?;
            sectors.push(sector);
        }

        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::EXTENSION);

        let mut extension = vec![0u8; header.length as usize];
        cursor.read_exact(&mut extension)?;

        Ok(Self {
            header,
            root_is_world,
            world_origin_x,
            world_origin_y,
            world_origin_z,
            triangle_count,
            vertices_count,
            unknown,
            count_sector,
            unknown2,
            flag,
            bbox_max,
            bbox_min,

            materials,
            sectors,
            extension,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct Sector {
    pub bbox_min: Vec3f,
    pub bbox_max: Vec3f,
    pub vertices: Vec<Vec3f>,
    pub normals: Option<Vec<Vec3f>>,
    pub texcoords: Option<Vec<TexCoord>>,
    pub triangles: Vec<Triangle>,
    pub extension: Vec<u8>,
}

impl Sector {
    pub fn read_header(cursor: &mut dyn Read) -> anyhow::Result<ChunkHeader> {
        let header = ChunkHeader::read(cursor)?;
        println!("good2");
        check_ty!(header.ty, ChunkType::ATOMIC_SECTOR);

        println!("good2.1");

        Ok(header)
    }

    pub fn read(cursor: &mut dyn Read, flag: FormatFlag) -> anyhow::Result<Self> {
        println!("good2.5");
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRUCT);

        println!("good3");

        let _material_id_base = cursor.read_u32_le()?;
        let triangle_count = cursor.read_u32_le()?;
        let vertex_count = cursor.read_u32_le()?;
        let bbox_min = Vec3f::read(cursor)?;
        let bbox_max = Vec3f::read(cursor)?;
        let _unknown = cursor.read_u32_le()?;
        let _unknown2 = cursor.read_u32_le()?;

        let mut vertices = vec![];
        for _ in 0..vertex_count {
            vertices.push(Vec3f::read(cursor)?);
        }

        let normals = if flag.contains(FormatFlag::NORMALS) {
            let mut normals = vec![];
            for _ in 0..vertex_count {
                normals.push(Vec3f::read(cursor)?);
            }

            Some(normals)
        } else {
            None
        };

        let texcoords =
            if flag.contains(FormatFlag::TEXTURED) || flag.contains(FormatFlag::TEXTURED2) {
                let mut texcoords = vec![];
                for _ in 0..vertex_count {
                    texcoords.push(TexCoord::read(cursor)?);
                }

                Some(texcoords)
            } else {
                None
            };

        let mut triangles = vec![];
        for _ in 0..triangle_count {
            let i0 = cursor.read_u16_le()?;
            let i1 = cursor.read_u16_le()?;
            let i2 = cursor.read_u16_le()?;
            let material = cursor.read_u16_le()?;
            triangles.push(Triangle {
                index: [i0, i1, i2],
                material,
            });
        }

        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::EXTENSION);

        let mut extension = vec![0u8; header.length as usize];
        cursor.read_exact(&mut extension)?;

        Ok(Self {
            bbox_min,
            bbox_max,
            vertices,
            normals,
            texcoords,
            triangles,
            extension,
        })
    }
}
