use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use super::{
    check_ty,
    extension::Extension,
    material::Material,
    sector::{AtomicSector, PlaneSector, Sector},
    ChunkHeader, ChunkType, FormatFlag, Vec3f,
};

#[derive(Debug, Serialize)]
pub struct World {
    pub header: ChunkHeader,
    root_type: u32,
    pub world_origin_x: f32,
    pub world_origin_y: f32,
    pub world_origin_z: f32,
    triangle_count: u32,
    vertices_count: u32,
    count_plane: u32,
    count_sector: u32,
    unknown2: u32,
    flag: FormatFlag,
    pub bbox_max: Vec3f,
    pub bbox_min: Vec3f,

    pub materials: Vec<Material>,
    pub sector: Sector,
    pub extensions: Vec<Extension>,
}

impl World {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::STRUCT);

        let root_type = cursor.read_u32_le()?;
        let world_origin_x = -cursor.read_f32::<LittleEndian>()?;
        let world_origin_y = -cursor.read_f32::<LittleEndian>()?;
        let world_origin_z = -cursor.read_f32::<LittleEndian>()?;

        let triangle_count = cursor.read_u32_le()?;
        let vertices_count = cursor.read_u32_le()?;
        let count_plane = cursor.read_u32_le()?;

        let count_sector = cursor.read_u32_le()?;
        let unknown2 = cursor.read_u32_le()?;
        let flag = FormatFlag(cursor.read_u32_le()?);
        let bbox_max = Vec3f::read(cursor)?;
        let bbox_min = Vec3f::read(cursor)?;

        let _ = super::material::read_material_list_header(cursor)?;
        let materials = super::material::read_material_list(cursor)?;

        let sector = if root_type == 0 {
            let _ = PlaneSector::read_header(cursor)?;
            let plane = PlaneSector::read(cursor, flag)?;
            Sector::PlaneSector(plane)
        } else {
            let _ = AtomicSector::read_header(cursor)?;
            let atomic = AtomicSector::read(cursor, flag)?;
            Sector::AtomicSector(atomic)
        };

        let extensions = Extension::read(cursor, 0)?;

        Ok(Self {
            header,
            root_type,
            world_origin_x,
            world_origin_y,
            world_origin_z,
            triangle_count,
            vertices_count,
            count_plane,
            count_sector,
            unknown2,
            flag,
            bbox_max,
            bbox_min,

            materials,
            sector,
            extensions,
        })
    }
}
