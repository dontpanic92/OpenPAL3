use binrw::binrw;
use serde::Serialize;

use crate::utils::StringWithCapacity;

#[binrw]
#[brw(little, magic = 0x0001e240u32)]
#[derive(Debug, Serialize)]
pub struct NodFile {
    pub version: u32,
    pub node_count: u32,

    // version < 9 is not supported
    #[br(if(version >= 9), count = node_count)]
    pub nodes: Vec<Node>,

    pub unknown: u32,
    pub unknown2: u32,
}

#[binrw]
#[brw(little)]
#[derive(Debug, Serialize)]
pub struct Node {
    #[brw(args(100))]
    pub name: StringWithCapacity,

    pub position: [f32; 3],
    pub scale: [f32; 3],
    pub rotation: [f32; 3],
    pub unknown_f32: [f32; 3],
    pub id: u32,
    pub asset_id: u32,
    pub unknown2: u32,
    pub aabb: [f32; 6],
    pub unknown3: [u32; 7],
}
