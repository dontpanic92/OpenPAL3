use binrw::BinRead;
use serde::Serialize;
use std::io::{Read, Seek};

use crate::{
    rwbs::TexCoord,
    utils::{SizedString, StringWithCapacity},
};

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3Texture {
    #[br(count = 17)]
    pub unknown: Vec<f32>,
    #[br(count = 4)]
    pub names: Vec<SizedString>,
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3Vertex {
    #[br(map = |v: i16| -v)]
    pub x: i16,
    pub y: i16,
    #[br(map = |v: i16| -v)]
    pub z: i16,
    pub normal_phi: i8,
    pub normal_theta: u8,
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little, import(count: u32))]
pub struct Mv3Frame {
    pub timestamp: u32,
    #[br(count = count)]
    pub vertices: Vec<Mv3Vertex>,
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3Triangle {
    pub indices: [u16; 3],
    pub texcoord_indices: [u16; 3],
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3UnknownDataInMesh {
    pub u: u16,
    pub v: u16,
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3Mesh {
    pub unknown: u32,
    pub triangle_count: u32,
    #[br(count = triangle_count)]
    pub triangles: Vec<Mv3Triangle>,
    pub unknown_data_count: u32,
    #[br(count = unknown_data_count)]
    pub unknown_data: Vec<Mv3UnknownDataInMesh>,
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3Model {
    #[br(count = 64)]
    pub unknown: Vec<u8>,
    pub vertex_per_frame: u32,
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],
    pub frame_count: u32,
    #[br(count = frame_count, args { inner: (vertex_per_frame,) })]
    pub frames: Vec<Mv3Frame>,
    pub texcoord_count: u32,
    #[br(count = texcoord_count)]
    pub texcoords: Vec<TexCoord>,
    pub mesh_count: u32,
    #[br(count = mesh_count)]
    pub meshes: Vec<Mv3Mesh>,
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3ActionDesc {
    pub tick: u32,
    #[brw(args(16))]
    pub name: StringWithCapacity,
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3UnknownDataInFile {
    #[br(count = 64)]
    pub unknown0: Vec<u8>,
    pub unknown1: u32,
    pub unknown2_count: u32,
    #[br(count = unknown2_count)]
    pub unknown2: Vec<[f32; 17]>,
}

#[derive(BinRead, Debug, Serialize, Clone)]
#[brw(little, magic = b"MV3\0")]
pub struct Mv3File {
    pub version: u32,
    pub duration: u32,

    pub texture_count: u32,
    pub unknown_data_count: u32,
    pub model_count: u32,
    pub action_count: u32,

    #[br(count = action_count)]
    pub action_desc: Vec<Mv3ActionDesc>,
    #[br(count = unknown_data_count)]
    pub unknown_data: Vec<Mv3UnknownDataInFile>,
    #[br(count = texture_count)]
    pub textures: Vec<Mv3Texture>,
    #[br(count = model_count)]
    pub models: Vec<Mv3Model>,
}

pub fn read_mv3(reader: &mut (impl Read + Seek)) -> anyhow::Result<Mv3File> {
    Ok(Mv3File::read(reader)?)
}
