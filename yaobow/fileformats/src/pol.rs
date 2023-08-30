use binrw::BinRead;
use serde::Serialize;
use std::io::{Read, Seek};

use crate::{
    rwbs::{Matrix44f, TexCoord, Vec3f},
    utils::{SizedString, StringWithCapacity},
};

#[derive(BinRead, Debug, Serialize, Clone, Copy)]
#[brw(little)]
pub struct PolVertexComponents(u32);
impl PolVertexComponents {
    pub const POSITION: Self = PolVertexComponents(0b1);
    pub const NORMAL: Self = PolVertexComponents(0b10);
    pub const UNKNOWN4: Self = PolVertexComponents(0b100);
    pub const UNKNOWN8: Self = PolVertexComponents(0b1000);
    pub const TEXCOORD: Self = PolVertexComponents(0b10000);
    pub const TEXCOORD2: Self = PolVertexComponents(0b100000);
    pub const UNKNOWN40: Self = PolVertexComponents(0b1000000);
    pub const UNKNOWN80: Self = PolVertexComponents(0b10000000);
    pub const UNKNOWN100: Self = PolVertexComponents(0b100000000);

    pub fn has(&self, c: PolVertexComponents) -> bool {
        (self.0 & c.0) != 0
    }
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little)]
pub struct PolVertexPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little, import(t: PolVertexComponents))]
pub struct PolVertex {
    pub position: Vec3f,
    #[br(if(t.has(PolVertexComponents::NORMAL)))]
    pub normal: Option<Vec3f>,
    #[br(if(t.has(PolVertexComponents::UNKNOWN4)))]
    pub unknown4: Option<f32>,
    #[br(if(t.has(PolVertexComponents::UNKNOWN8)))]
    pub unknown8: Option<f32>,
    pub tex_coord: TexCoord,
    #[br(if(t.has(PolVertexComponents::TEXCOORD2)))]
    pub tex_coord2: Option<TexCoord>,
    #[br(if(t.has(PolVertexComponents::UNKNOWN40)))]
    pub unknown40: Option<[f32; 2]>,
    #[br(if(t.has(PolVertexComponents::UNKNOWN80)))]
    pub unknown80: Option<[f32; 2]>,
    #[br(if(t.has(PolVertexComponents::UNKNOWN100)))]
    pub unknown100: Option<[f32; 4]>,
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little)]
pub struct PolMaterialInfo {
    pub use_alpha: u32,
    #[br(count = 16)]
    pub unknown_68: Vec<f32>,
    pub unknown_float: f32,
    pub texture_count: u32,
    #[br(count = texture_count, args { inner: (64,) })]
    pub texture_names: Vec<StringWithCapacity>,
    pub unknown2: u32,
    pub unknown3: u32,
    pub unknown4: u32,
    pub triangle_count: u32,
    #[br(count = triangle_count)]
    pub triangles: Vec<PolTriangle>,
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little)]
pub struct PolTriangle {
    pub indices: [u16; 3],
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(
    little,
    assert(
        vertex_type.has(PolVertexComponents::POSITION)
        && vertex_type.has(PolVertexComponents::TEXCOORD),
        "This POL file doesn't have POSITION or TEXCOORD info, which doesn't support currently."
    )
)]
pub struct PolMesh {
    pub aabb_min: Vec3f,
    pub aabb_max: Vec3f,
    pub vertex_type: PolVertexComponents,
    pub vertex_count: u32,
    #[br(count = vertex_count as usize, args { inner: (vertex_type,) })]
    pub vertices: Vec<PolVertex>,
    pub material_info_count: u32,
    #[br(count = material_info_count)]
    pub material_info: Vec<PolMaterialInfo>,
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little)]
pub struct UnknownData {
    #[br(count = 32)]
    pub unknown: Vec<u8>, // size: 32
    pub matrix: Matrix44f,
    pub unknown2: u32,
    pub ddd_str: SizedString,
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little)]
pub struct GeomNodeDesc {
    #[br(count = 26)]
    pub unknown: Vec<u16>, // size: 52
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little, magic = b"POLY")]
pub struct PolFile {
    pub some_flag: u32,
    pub mesh_count: u32,
    #[br(count = mesh_count)]
    pub geom_node_descs: Vec<GeomNodeDesc>,
    #[br(if(some_flag > 100))]
    pub unknown_count: u32,
    #[br(if(some_flag > 100), count = unknown_count)]
    pub unknown_data: Vec<UnknownData>,
    #[br(count = mesh_count)]
    pub meshes: Vec<PolMesh>,
}

pub fn read_pol(reader: &mut (impl Read + Seek)) -> anyhow::Result<PolFile> {
    Ok(PolFile::read(reader)?)
}
