use crate::utilities::ReadExt;
use byteorder::{LittleEndian, ReadBytesExt};
use mini_fs::{MiniFs, StoreExt};
use radiance::math::Vec3;
use serde::Serialize;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Copy, Clone, Serialize)]
pub struct NavMapPoint {
    pub height: f32,
    pub distance_to_border: u32,
}

#[derive(Debug, Serialize)]
pub struct NavUnknown22 {
    pub unknown1: u16,
    pub unknown2: u16,
    pub unknown3: u16,
}

#[derive(Debug, Serialize)]
pub struct NavMap {
    pub unknown: Option<Vec<u32>>, // count: 32
    pub max_coord: Vec3,
    pub min_coord: Vec3,
    pub width: u32,
    pub height: u32,
    pub map: Vec<Vec<NavMapPoint>>,
}

#[derive(Debug, Serialize)]
pub struct NavUnknown2 {
    pub unknown21: Vec<Vec3>,
    pub unknown22: Vec<NavUnknown22>,
}

#[derive(Debug, Serialize)]
pub struct NavFile {
    pub version: u32,
    pub maps: Vec<NavMap>,
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
    let mut maps = vec![];
    for _ in 0..count {
        maps.push(nav_read_unknown1(&mut reader, version));
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
        maps,
        unknown2,
    }
}

fn nav_read_unknown1(reader: &mut dyn Read, version: u8) -> NavMap {
    let mut unknown = None;
    if version == 2 {
        unknown = Some(reader.read_dw_vec(32).unwrap());
    }

    let max_coord_x = reader.read_f32::<LittleEndian>().unwrap();
    let max_coord_y = reader.read_f32::<LittleEndian>().unwrap();
    let max_coord_z = reader.read_f32::<LittleEndian>().unwrap();
    let min_coord_x = reader.read_f32::<LittleEndian>().unwrap();
    let min_coord_y = reader.read_f32::<LittleEndian>().unwrap();
    let min_coord_z = reader.read_f32::<LittleEndian>().unwrap();
    let width = reader.read_u32::<LittleEndian>().unwrap();
    let height = reader.read_u32::<LittleEndian>().unwrap();
    let mut map = vec![];
    for _ in 0..height {
        let mut tmp = vec![];
        for _ in 0..width {
            let height = reader.read_f32::<LittleEndian>().unwrap();
            let distance_to_border = reader.read_u32::<LittleEndian>().unwrap();
            tmp.push(NavMapPoint {
                height,
                distance_to_border,
            })
        }

        map.push(tmp);
    }

    NavMap {
        unknown,
        max_coord: Vec3::new(max_coord_x, max_coord_y, max_coord_z),
        min_coord: Vec3::new(min_coord_x, min_coord_y, min_coord_z),
        width,
        height,
        map,
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
