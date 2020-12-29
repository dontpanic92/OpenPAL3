use crate::utilities::ReadExt;
use byteorder::{LittleEndian, ReadBytesExt};
use mini_fs::{MiniFs, StoreExt};
use radiance::math::Vec3;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug)]
pub struct NavUnknown11 {
    pub f: f32,
    pub dw: u32,
}

#[derive(Debug)]
pub struct NavUnknown22 {
    pub unknown1: u16,
    pub unknown2: u16,
    pub unknown3: u16,
}

#[derive(Debug)]
pub struct NavUnknown1 {
    pub unknown: Option<Vec<u32>>, // count: 32
    pub unknown_vec: Vec3,
    pub origin: Vec3,
    pub width: u32,
    pub height: u32,
    pub unknown1: Vec<Vec<NavUnknown11>>,
}

#[derive(Debug)]
pub struct NavUnknown2 {
    pub unknown21: Vec<Vec3>,
    pub unknown22: Vec<NavUnknown22>,
}

#[derive(Debug)]
pub struct NavFile {
    pub version: u32,
    pub unknown1: Vec<NavUnknown1>,
    pub unknown2: Vec<NavUnknown2>,
}

pub fn nav_load_from_file<P: AsRef<Path>>(vfs: &MiniFs, path: P) -> NavFile {
    let mut reader = BufReader::new(vfs.open(path).unwrap());
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).unwrap();

    match magic {
        [0x4e, 0x41, 0x56, 0x00] => (), // "nav"
        _ => panic!("Not a valid nav file"),
    };

    let version = reader.read_u8().unwrap();
    if version != 1 && version != 2 {
        panic!("nav version should be 1 or 2");
    }

    let count = reader.read_u8().unwrap();
    let unknown1_offset = reader.read_u32::<LittleEndian>().unwrap();
    let unknown2_offset = reader.read_u32::<LittleEndian>().unwrap();

    reader
        .seek(SeekFrom::Start(unknown1_offset as u64))
        .unwrap();
    let mut unknown1 = vec![];
    for _ in 0..count {
        unknown1.push(nav_read_unknown1(&mut reader, version));
    }

    reader
        .seek(SeekFrom::Start(unknown2_offset as u64))
        .unwrap();
    let mut unknown2 = vec![];
    for _ in 0..count {
        unknown2.push(nav_read_unknown2(&mut reader, version));
    }

    NavFile {
        version: version as u32,
        unknown1,
        unknown2,
    }
}

fn nav_read_unknown1(reader: &mut dyn Read, version: u8) -> NavUnknown1 {
    let mut unknown = None;
    if version == 2 {
        unknown = Some(reader.read_dw_vec(32).unwrap());
    }

    let vec1_x = reader.read_f32::<LittleEndian>().unwrap();
    let vec1_y = reader.read_f32::<LittleEndian>().unwrap();
    let vec1_z = reader.read_f32::<LittleEndian>().unwrap();
    let vec2_x = reader.read_f32::<LittleEndian>().unwrap();
    let vec2_y = reader.read_f32::<LittleEndian>().unwrap();
    let vec2_z = reader.read_f32::<LittleEndian>().unwrap();
    let width = reader.read_u32::<LittleEndian>().unwrap();
    let height = reader.read_u32::<LittleEndian>().unwrap();
    let mut unknown1 = vec![];
    for _ in 0..width {
        let mut tmp = vec![];
        for _ in 0..height {
            let f = reader.read_f32::<LittleEndian>().unwrap();
            let dw = reader.read_u32::<LittleEndian>().unwrap();
            tmp.push(NavUnknown11 { f, dw })
        }

        unknown1.push(tmp);
    }

    NavUnknown1 {
        unknown,
        unknown_vec: Vec3::new(vec1_x, vec1_y, vec1_z),
        origin: Vec3::new(vec2_x, vec2_y, vec2_z),
        width,
        height,
        unknown1,
    }
}

fn nav_read_unknown2(reader: &mut dyn Read, version: u8) -> NavUnknown2 {
    let count1 = reader.read_u16::<LittleEndian>().unwrap();
    let count2 = reader.read_u16::<LittleEndian>().unwrap();
    let mut unknown21 = vec![];
    for _ in 0..count1 {
        let x = reader.read_f32::<LittleEndian>().unwrap();
        let y = reader.read_f32::<LittleEndian>().unwrap();
        let z = reader.read_f32::<LittleEndian>().unwrap();
        unknown21.push(Vec3::new(x, y, z))
    }

    let mut unknown22 = vec![];
    for _ in 0..count2 {
        let unknown1 = reader.read_u16::<LittleEndian>().unwrap();
        let unknown2 = reader.read_u16::<LittleEndian>().unwrap();
        let unknown3 = reader.read_u16::<LittleEndian>().unwrap();
        unknown22.push(NavUnknown22 {
            unknown1,
            unknown2,
            unknown3,
        })
    }

    NavUnknown2 {
        unknown21,
        unknown22,
    }
}
