use super::{read_dw_vec, read_f32_vec, read_string, read_vec, read_w_vec};
use byteorder::{LittleEndian, ReadBytesExt};
use radiance::math::Vec3;
use std::error::Error;
use std::fs;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct ScnNode {
    pub d0: u32,
    pub name: String,
    pub dw24: u32,
    pub position: Vec3,
    pub rotation: f32,
    pub dw38: Vec<u32>,
    pub node_type: u8,
    pub b49: Vec<u8>,
    pub w66: Vec<u16>,
    pub b6e: Vec<u8>,
    pub d80: Vec<u32>,
    pub b88: Vec<u8>,
    pub w148: u16,
    pub b14a: Vec<u8>,
    pub vec1: Vec3,
    pub vec2: Vec3,
    pub dw184: Vec<u32>,
    pub b: Vec<u8>,
}

#[derive(Debug)]
pub struct ScnFile {
    pub cpk_name: String,
    pub scn_name: String,
    pub unknown_data: Vec<Vec<u8>>,
    pub nodes: Vec<ScnNode>,
}

pub fn scn_load_from_file<P: AsRef<Path>>(path: P) -> ScnFile {
    let mut reader = BufReader::new(fs::File::open(path).unwrap());
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

    let unknown_data_num = reader.read_u16::<LittleEndian>().unwrap();
    let unknown_data_offset = reader.read_u32::<LittleEndian>().unwrap();
    let node_num = reader.read_u16::<LittleEndian>().unwrap();
    let node_offset = reader.read_u32::<LittleEndian>().unwrap();

    let cpk_name = read_string(&mut reader, 32).unwrap();
    let scn_name = read_string(&mut reader, 64).unwrap();

    let mut unknown_data = vec![];
    reader
        .seek(SeekFrom::Start(unknown_data_offset as u64))
        .unwrap();
    for _i in 0..unknown_data_num {
        let v = read_vec(&mut reader, 456).unwrap();
        unknown_data.push(v);
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
        unknown_data,
        nodes,
    }
}

fn read_scn_node(reader: &mut dyn Read) -> ScnNode {
    let d0 = reader.read_u32::<LittleEndian>().unwrap();
    let name = read_string(reader, 32).unwrap();
    let dw24 = reader.read_u32::<LittleEndian>().unwrap();
    let position_x = reader.read_f32::<LittleEndian>().unwrap();
    let position_y = reader.read_f32::<LittleEndian>().unwrap();
    let position_z = reader.read_f32::<LittleEndian>().unwrap();
    let rotation = reader.read_f32::<LittleEndian>().unwrap();
    let dw38 = read_dw_vec(reader, 4).unwrap();
    let node_type = reader.read_u8().unwrap();
    let b49 = read_vec(reader, 29).unwrap();
    let w66 = read_w_vec(reader, 4).unwrap();
    let b6e = read_vec(reader, 18).unwrap();
    let d80 = read_dw_vec(reader, 2).unwrap();
    let b88 = read_vec(reader, 192).unwrap();
    let w148 = reader.read_u16::<LittleEndian>().unwrap();
    let b14a = read_vec(reader, 34).unwrap();
    let vec1_x = reader.read_f32::<LittleEndian>().unwrap();
    let vec1_y = reader.read_f32::<LittleEndian>().unwrap();
    let vec1_z = reader.read_f32::<LittleEndian>().unwrap();
    let vec2_x = reader.read_f32::<LittleEndian>().unwrap();
    let vec2_y = reader.read_f32::<LittleEndian>().unwrap();
    let vec2_z = reader.read_f32::<LittleEndian>().unwrap();
    let dw184 = read_dw_vec(reader, 6).unwrap();
    let b = read_vec(reader, 208).unwrap();

    ScnNode {
        d0,
        name,
        dw24,
        position: Vec3::new(position_x, position_y, position_z),
        rotation,
        dw38,
        node_type,
        b49,
        w66,
        b6e,
        d80,
        b88,
        w148,
        b14a,
        vec1: Vec3::new(vec1_x, vec1_y, vec1_z),
        vec2: Vec3::new(vec2_x, vec2_y, vec2_z),
        dw184,
        b,
    }
}
