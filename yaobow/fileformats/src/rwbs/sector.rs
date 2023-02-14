use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::{
    check_ty, extension::Extension, ChunkHeader, ChunkType, FormatFlag, Normal, PrelitColor,
    TexCoord, Triangle, Vec3f,
};

#[derive(Debug, Serialize)]
pub enum Sector {
    PlaneSector(PlaneSector),
    AtomicSector(AtomicSector),
}

#[derive(Debug, Serialize)]
pub struct PlaneSector {
    pub sector_type: u32,
    pub unknown: u32,
    pub value: f32,
    pub unknown2: f32,
    pub unknown3: u32,
    pub left_child_type: u32,
    pub right_child_type: u32,
    pub left_value: f32,
    pub right_value: f32,
    pub left_child: Box<Sector>,
    pub right_child: Box<Sector>,
}

impl PlaneSector {
    pub fn read_header(cursor: &mut dyn Read) -> anyhow::Result<ChunkHeader> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::PLANE_SECTOR);

        Ok(header)
    }

    pub fn read(cursor: &mut dyn Read, flag: FormatFlag) -> anyhow::Result<Self> {
        let sector_type = cursor.read_u32_le()?;
        let unknown = cursor.read_u32_le()?;
        let unknown2 = cursor.read_f32::<LittleEndian>()?;
        let unknown3 = cursor.read_u32_le()?;
        let value = cursor.read_f32::<LittleEndian>()?;
        let left_child_type = cursor.read_u32_le()?;
        let right_child_type = cursor.read_u32_le()?;
        let left_value = cursor.read_f32::<LittleEndian>()?;
        let right_value = cursor.read_f32::<LittleEndian>()?;

        let left_child = if left_child_type != 0 {
            let _ = AtomicSector::read_header(cursor)?;
            let sector = AtomicSector::read(cursor, flag)?;
            Box::new(Sector::AtomicSector(sector))
        } else {
            let _ = PlaneSector::read_header(cursor)?;
            let plane = PlaneSector::read(cursor, flag)?;
            Box::new(Sector::PlaneSector(plane))
        };

        let right_child = if right_child_type != 0 {
            let _ = AtomicSector::read_header(cursor)?;
            let sector = AtomicSector::read(cursor, flag)?;
            Box::new(Sector::AtomicSector(sector))
        } else {
            let _ = PlaneSector::read_header(cursor)?;
            let plane = PlaneSector::read(cursor, flag)?;
            Box::new(Sector::PlaneSector(plane))
        };

        Ok(Self {
            sector_type,
            unknown,
            value,
            unknown2,
            unknown3,
            left_child_type,
            right_child_type,
            left_value,
            right_value,
            left_child,
            right_child,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct AtomicSector {
    pub bbox_min: Vec3f,
    pub bbox_max: Vec3f,
    pub vertices: Vec<Vec3f>,
    pub prelit_colors: Option<Vec<PrelitColor>>,
    pub normals: Option<Vec<Normal>>,
    pub texcoords: Option<Vec<TexCoord>>,
    pub texcoords2: Option<Vec<TexCoord>>,
    pub triangles: Vec<Triangle>,
    pub extensions: Vec<Extension>,
}

impl AtomicSector {
    pub fn read_header(cursor: &mut dyn Read) -> anyhow::Result<ChunkHeader> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::ATOMIC_SECTOR);

        Ok(header)
    }

    pub fn read(cursor: &mut dyn Read, flag: FormatFlag) -> anyhow::Result<Self> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRUCT);

        let _material_id_base = cursor.read_u32_le()?;
        let triangle_count = cursor.read_u32_le()?;
        let vertices_count = cursor.read_u32_le()?;
        let bbox_min = Vec3f::read(cursor)?;
        let bbox_max = Vec3f::read(cursor)?;
        let _unknown = cursor.read_u32_le()?;
        let _unknown2 = cursor.read_u32_le()?;

        let mut vertices = vec![];
        for _ in 0..vertices_count {
            vertices.push(Vec3f::read(cursor)?);
        }

        let prelit_colors = if flag.contains(FormatFlag::PRELIT) {
            let mut colors = vec![];
            for _ in 0..vertices_count {
                colors.push(PrelitColor::read(cursor)?);
            }

            Some(colors)
        } else {
            None
        };

        let normals = if flag.contains(FormatFlag::NORMALS) {
            let mut normals = vec![];
            for _ in 0..vertices_count {
                normals.push(Normal::read(cursor)?);
            }

            Some(normals)
        } else {
            None
        };

        let texcoords =
            if flag.contains(FormatFlag::TEXTURED) || flag.contains(FormatFlag::TEXTURED2) {
                let mut texcoords = vec![];
                for _ in 0..vertices_count {
                    texcoords.push(TexCoord::read(cursor)?);
                }

                Some(texcoords)
            } else {
                None
            };

        let texcoords2 = if flag.contains(FormatFlag::TEXTURED2) {
            let mut texcoords = vec![];
            for _ in 0..vertices_count {
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

        let extensions = Extension::read(cursor, vertices_count)?;

        Ok(Self {
            bbox_min,
            bbox_max,
            vertices,
            prelit_colors,
            normals,
            texcoords,
            texcoords2,
            triangles,
            extensions,
        })
    }
}
