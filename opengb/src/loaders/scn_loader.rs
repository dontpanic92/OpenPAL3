use crate::utilities::ReadExt;
use byteorder::{LittleEndian, ReadBytesExt};
use mini_fs::{MiniFs, StoreExt};
use radiance::math::Vec3;
use serde::Serialize;
use std::{
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

#[derive(Debug, Clone, Serialize)]
pub struct ScnNode {
    pub index: u16,
    pub w2: u16,
    pub name: String,
    pub w24: u16,
    pub w26: u16,
    pub position: Vec3,
    pub rotation: f32,
    pub nav_trigger_coord_min: (u32, u32),
    pub nav_trigger_coord_max: (u32, u32),
    pub node_type: u8,
    pub b49: Vec<u8>,
    pub w66: Vec<u16>,
    pub b6e: Vec<u8>,
    pub sce_proc_id: u32,
    pub d84: u32,
    pub b88: Vec<u8>,
    pub w148: u16,
    pub b14a: Vec<u8>,
    pub aabb_trigger_coord1: Vec3,
    pub aabb_trigger_coord2: Vec3,
    pub dw184: Vec<u32>,
    pub b: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScnRole {
    pub index: u8,
    pub b1: u8,
    pub name: String,
    pub w42: u16,
    pub dw44: f32,
    pub dw48: u32,
    pub position_x: f32,
    pub position_z: f32,
    pub position_y: f32,
    pub dw58: u32,
    pub sce_proc_id: u32,
    pub dw60: u32,
    pub action_name: String,
    pub dw74: u32,
    pub dw78: u32,
    pub dw7c: u32,
    pub b80: Vec<u8>,
    pub dw84: Vec<u32>,
    pub dw148: u32,
    pub dw14c: u32,
    pub dw150: u32,
    pub dw154: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScnFile {
    pub cpk_name: String,
    pub scn_name: String,
    pub scn_base_name: String,
    pub roles: Vec<ScnRole>,
    pub nodes: Vec<ScnNode>,
}

pub fn scn_load_from_file<P: AsRef<Path>>(vfs: &MiniFs, path: P) -> ScnFile {
    let mut reader = BufReader::new(vfs.open(path).unwrap());
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).unwrap();

    match magic {
        [0x53, 0x43, 0x4e, 0x00] => (), // "SCN"
        _ => panic!("Not a valid scn file"),
    }

    let magic2 = reader.read_u16::<LittleEndian>().unwrap();
    if magic2 != 1 {
        panic!("Not a valid scn file");
    }

    let role_num = reader.read_u16::<LittleEndian>().unwrap();
    let role_offset = reader.read_u32::<LittleEndian>().unwrap();
    let node_num = reader.read_u16::<LittleEndian>().unwrap();
    let node_offset = reader.read_u32::<LittleEndian>().unwrap();

    let cpk_name = reader.read_string(32).unwrap();
    let scn_name = reader.read_string(32).unwrap();
    let scn_base_name = reader.read_string(32).unwrap();

    let mut roles = vec![];
    reader.seek(SeekFrom::Start(role_offset as u64)).unwrap();
    for _i in 0..role_num {
        let v = read_scn_role(&mut reader);
        roles.push(v);
    }

    let mut nodes = vec![];
    reader.seek(SeekFrom::Start(node_offset as u64)).unwrap();
    for _i in 0..node_num {
        let node = read_scn_node(&mut reader);
        nodes.push(node);
    }

    ScnFile {
        cpk_name,
        scn_name,
        scn_base_name,
        roles,
        nodes,
    }
}

fn read_scn_role(reader: &mut dyn Read) -> ScnRole {
    let index = reader.read_u8().unwrap();
    let b1 = reader.read_u8().unwrap();
    let name = reader.read_string(64).unwrap();
    let w42 = reader.read_u16::<LittleEndian>().unwrap();
    let dw44 = reader.read_f32::<LittleEndian>().unwrap();
    let dw48 = reader.read_u32::<LittleEndian>().unwrap();
    let position_x = reader.read_f32::<LittleEndian>().unwrap();
    let position_z = reader.read_f32::<LittleEndian>().unwrap();
    let position_y = reader.read_f32::<LittleEndian>().unwrap();
    let dw58 = reader.read_u32::<LittleEndian>().unwrap();
    let sce_proc_id = reader.read_u32::<LittleEndian>().unwrap();
    let dw60 = reader.read_u32::<LittleEndian>().unwrap();
    let action_name = reader.read_string(16).unwrap();
    let dw74 = reader.read_u32::<LittleEndian>().unwrap();
    let dw78 = reader.read_u32::<LittleEndian>().unwrap();
    let dw7c = reader.read_u32::<LittleEndian>().unwrap();
    let b80 = reader.read_u8_vec(4).unwrap();
    let dw84 = reader.read_dw_vec(49).unwrap();
    let dw148 = reader.read_u32::<LittleEndian>().unwrap();
    let dw14c = reader.read_u32::<LittleEndian>().unwrap();
    let dw150 = reader.read_u32::<LittleEndian>().unwrap();
    let dw154 = reader.read_dw_vec(29).unwrap();

    ScnRole {
        index,
        b1,
        name,
        w42,
        dw44,
        dw48,
        position_x,
        position_z,
        position_y,
        dw58,
        sce_proc_id,
        dw60,
        action_name,
        dw74,
        dw78,
        dw7c,
        b80,
        dw84,
        dw148,
        dw14c,
        dw150,
        dw154,
    }
}

fn read_scn_node(reader: &mut dyn Read) -> ScnNode {
    let index = reader.read_u16::<LittleEndian>().unwrap();
    let w2 = reader.read_u16::<LittleEndian>().unwrap();
    let name = reader.read_string(32).unwrap();
    let w24 = reader.read_u16::<LittleEndian>().unwrap();
    let w26 = reader.read_u16::<LittleEndian>().unwrap();
    let position_x = reader.read_f32::<LittleEndian>().unwrap();
    let position_y = reader.read_f32::<LittleEndian>().unwrap();
    let position_z = reader.read_f32::<LittleEndian>().unwrap();
    let rotation = reader.read_f32::<LittleEndian>().unwrap();
    let nav_coord_min_x = reader.read_u32::<LittleEndian>().unwrap();
    let nav_coord_min_z = reader.read_u32::<LittleEndian>().unwrap();
    let nav_coord_max_x = reader.read_u32::<LittleEndian>().unwrap();
    let nav_coord_max_z = reader.read_u32::<LittleEndian>().unwrap();
    let node_type = reader.read_u8().unwrap();
    let b49 = reader.read_u8_vec(29).unwrap();
    let w66 = reader.read_w_vec(4).unwrap();
    let b6e = reader.read_u8_vec(18).unwrap();
    let sce_proc_id = reader.read_u32::<LittleEndian>().unwrap();
    let d84 = reader.read_u32::<LittleEndian>().unwrap();
    let b88 = reader.read_u8_vec(192).unwrap();
    let w148 = reader.read_u16::<LittleEndian>().unwrap();
    let b14a = reader.read_u8_vec(34).unwrap();
    let vec1_x = reader.read_f32::<LittleEndian>().unwrap();
    let vec1_y = reader.read_f32::<LittleEndian>().unwrap();
    let vec1_z = reader.read_f32::<LittleEndian>().unwrap();
    let vec2_x = reader.read_f32::<LittleEndian>().unwrap();
    let vec2_y = reader.read_f32::<LittleEndian>().unwrap();
    let vec2_z = reader.read_f32::<LittleEndian>().unwrap();
    let dw184 = reader.read_dw_vec(6).unwrap();
    let b = reader.read_u8_vec(208).unwrap();

    ScnNode {
        index,
        w2,
        name,
        w24,
        w26,
        position: Vec3::new(position_x, position_y, position_z),
        rotation,
        nav_trigger_coord_min: (nav_coord_min_x, nav_coord_min_z),
        nav_trigger_coord_max: (nav_coord_max_x, nav_coord_max_z),
        node_type,
        b49,
        w66,
        b6e,
        sce_proc_id,
        d84,
        b88,
        w148,
        b14a,
        aabb_trigger_coord1: Vec3::new(vec1_x, vec1_y, vec1_z),
        aabb_trigger_coord2: Vec3::new(vec2_x, vec2_y, vec2_z),
        dw184,
        b,
    }
}
