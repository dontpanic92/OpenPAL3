use std::io::SeekFrom;

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;

use crate::utils::{to_big5_string, SeekRead};

#[derive(Debug)]
pub struct MapsData {
    pub maps: Vec<MapData>,
}

impl MapsData {
    pub fn read(reader: &mut dyn SeekRead) -> anyhow::Result<Self> {
        let mut maps = Vec::new();

        reader.seek(SeekFrom::Start(8)).unwrap();
        let start = reader.read_u16_le()?;
        let start = start as u64 * 16;

        reader.seek(SeekFrom::Start(start))?;
        let _length = reader.read_u32_le()?;
        let map_start = reader.read_u32_le()?;
        let _movie_start = reader.read_u32_le()?;

        reader.seek(SeekFrom::Start(start + map_start as u64))?;
        loop {
            if let Some(map) = MapData::read(reader)? {
                maps.push(map);
            } else {
                break;
            }
        }

        Ok(Self { maps })
    }
}

#[derive(Debug)]
pub struct MapData {
    pub id: i16,
    pub unknown: [i16; 11],
    pub file_name: String,
    pub map_name: String,
}

impl MapData {
    pub fn read(reader: &mut dyn SeekRead) -> anyhow::Result<Option<Self>> {
        let id = reader.read_i16::<LittleEndian>()?;
        if id == -1 {
            return Ok(None);
        }

        let mut unknown = [0; 11];
        for i in 0..11 {
            unknown[i] = reader.read_i16::<LittleEndian>()?;
        }

        let file_name = Self::read_string(reader)?;
        let map_name = Self::read_string(reader)?;

        Ok(Some(Self {
            id,
            unknown,
            file_name,
            map_name,
        }))
    }

    fn read_string(reader: &mut dyn SeekRead) -> anyhow::Result<String> {
        let mut buf = vec![];
        loop {
            let sig = reader.read_u16_le()?;
            if sig == 0x5125 {
                break;
            }

            reader.seek(SeekFrom::Current(-2))?;
            let byte = reader.read_u8()?;
            buf.push(byte);
        }

        Ok(to_big5_string(&buf)?)
    }
}
