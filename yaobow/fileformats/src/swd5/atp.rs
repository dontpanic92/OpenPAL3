use std::{collections::HashMap, io::Cursor};

use binrw::BinRead;
use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use super::SizedBig5String;

#[derive(Debug)]
pub enum AtpHeaderItem {
    Description(SizedBig5String),
    EntryCount(u32),
    Dword(u32),
}

#[derive(Debug)]
pub struct AtpHeader {
    pub items: HashMap<i32, AtpHeaderItem>,
}

impl AtpHeader {
    pub fn read(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Self> {
        let mut items = HashMap::new();
        loop {
            let key = cursor.read_i32::<LittleEndian>()?;
            if key == -1 {
                break;
            }

            let item = match key {
                0 => AtpHeaderItem::Description(SizedBig5String::read(cursor)?),
                2 => AtpHeaderItem::EntryCount(cursor.read_u32::<LittleEndian>()?),
                _ => AtpHeaderItem::Dword(cursor.read_u32::<LittleEndian>()?),
            };

            items.insert(key, item);
        }

        Ok(AtpHeader { items })
    }

    pub fn get_description(&self) -> SizedBig5String {
        self.items
            .get(&0)
            .and_then(|item| match item {
                AtpHeaderItem::Description(desc) => Some(desc),
                _ => None,
            })
            .cloned()
            .unwrap_or(SizedBig5String {
                len: 0,
                data: vec![],
            })
    }

    pub fn get_entry_count(&self) -> u32 {
        self.items
            .get(&2)
            .and_then(|item| match item {
                AtpHeaderItem::EntryCount(count) => Some(*count),
                _ => None,
            })
            .unwrap_or(0)
    }
}

#[derive(Debug)]
pub struct AtpFile {
    pub header: AtpHeader,
    pub file_offsets: Vec<i32>,
    pub files: Vec<Option<AtpEntry>>,
}

impl AtpFile {
    pub fn read(data: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(data);
        let header = AtpHeader::read(&mut cursor)?;

        let mut file_offsets = vec![];

        for _ in 0..header.get_entry_count() {
            let offset = cursor.read_i32::<LittleEndian>()?;
            file_offsets.push(offset);
        }

        let position = cursor.position();

        let mut files = vec![];
        for offset in file_offsets.iter() {
            if *offset == -1 {
                files.push(None);
            } else {
                cursor.set_position(position + *offset as u64);
                files.push(Some(AtpEntry::read(&mut cursor)?));
            }
        }

        Ok(AtpFile {
            header,
            file_offsets,
            files,
        })
    }
}

macro_rules! read_data {
    ($struct_name: expr, $cursor: ident, $length: ident, $item_length: ident, $($index: expr => $action: expr, )*) => {
        let mut cur_length = 0;
        while cur_length < $length {
            $item_length = $cursor.read_u32_le()?;
            let index = $cursor.read_u16_le()?;
            // println!("index: {}", index);
            match index {
                $($index => $action,)*
                _ => {
                    log::warn!("Unknown index in {}: {} at position {}", $struct_name, index, $cursor.position());
                    $cursor.skip(($item_length - 2) as usize)?
                }
            }

            cur_length += $item_length + 4;
        }
    };
}

#[derive(Debug, Serialize)]
pub struct AtpEntry {
    pub unknown: Option<u32>,
    pub data2: Option<AtpEntryData2>,
    pub data3: Option<Vec<AtpEntryData3>>,
    pub data4: Option<AtpEntryData4>,
}

impl AtpEntry {
    pub fn read(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Self> {
        let length = cursor.read_u32_le()?;
        let mut cur_length = 0;
        let mut entry = AtpEntry {
            unknown: None,
            data2: None,
            data3: None,
            data4: None,
        };

        while cur_length < length {
            let item_length = cursor.read_u32_le()?;
            let index = cursor.read_u16_le()?;
            match index {
                1 => entry.unknown = Some(cursor.read_u32_le()?),
                2 => entry.data2 = Some(AtpEntryData2::read(cursor, item_length - 2)?),
                3 => entry.data3 = Some(read_data3(cursor)?),
                4 => {
                    entry.data4 = Some(AtpEntryData4::read(
                        cursor,
                        item_length - 2,
                        entry.unknown.unwrap(),
                    )?)
                }
                _ => {
                    log::warn!("Unknown index for AtpEntry: {}", index);
                    cursor.skip((item_length - 2) as usize)?
                }
            }

            cur_length += item_length + 4;
        }

        Ok(entry)
    }
}

#[derive(Debug, Serialize)]
pub struct AtpEntryData2 {
    pub name: Option<SizedBig5String>,
    pub unknown2: Option<u64>,
    pub unknown3: Option<Vec<u8>>,
    pub unknown4: Option<f32>,
    pub unknown5: Option<f32>,
    pub unknown6: Option<u8>,
}

impl AtpEntryData2 {
    pub fn read(cursor: &mut Cursor<&[u8]>, length: u32) -> anyhow::Result<Self> {
        let mut data = AtpEntryData2 {
            name: None,
            unknown2: None,
            unknown3: None,
            unknown4: None,
            unknown5: None,
            unknown6: None,
        };

        let mut _item_length = 0;
        read_data!(
            "AptEntryData2",
            cursor,
            length,
            _item_length,
            1 => data.name = Some(SizedBig5String::read(cursor)?),
            2 => data.unknown2 = Some(cursor.read_u64_le()?),
            3 => data.unknown3 = Some(cursor.read_u8_vec((_item_length - 2) as usize)?),
            4 => data.unknown4 = Some(cursor.read_f32::<LittleEndian>()?),
            5 => data.unknown5 = Some(cursor.read_f32::<LittleEndian>()?),
            6 => data.unknown6 = Some(cursor.read_u8()?),
        );

        Ok(data)
    }
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct AtpEntryData3 {
    pub dword: [u32; 7],
    pub float: [f32; 6],
}

fn read_data3(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Vec<AtpEntryData3>> {
    let mut data3 = vec![];
    let count = cursor.read_u32_le()?;
    for _ in 0..count {
        let item = AtpEntryData3::read(cursor)?;
        data3.push(item);
    }

    Ok(data3)
}

#[derive(Debug, Serialize)]
pub enum AtpEntryData4 {
    Data1(AtpEntryData41),
    Data2(AtpEntryData42),
    Data5(AtpEntryData45),
}

impl AtpEntryData4 {
    pub fn read(cursor: &mut Cursor<&[u8]>, length: u32, ty: u32) -> anyhow::Result<Self> {
        match ty {
            1 => Ok(AtpEntryData4::Data1(AtpEntryData41::read(cursor, length)?)),
            2 => Ok(AtpEntryData4::Data2(AtpEntryData42::read(cursor, length)?)),
            5 => Ok(AtpEntryData4::Data5(AtpEntryData45::read(cursor, length)?)),
            _ => Err(anyhow::anyhow!("Unknown AtpEntryData4 type: {}", ty)),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AtpEntryData41 {
    pub unknown1: Option<[f32; 3]>,
    pub unknown2: Option<u32>,
    pub file: Option<AtpEntryData41File>,
    pub sub_files: Option<Vec<AtpEntryData41SubFile>>,
    pub unknown5: Option<u32>,
    pub unknown6: Option<Vec<u8>>,
    pub unknown7: Option<u32>,
    pub unknown8: Option<u64>,
    pub unknown9: Option<u32>,
    pub unknown10: Option<u32>,
    pub unknown11: Option<u32>,
    pub unknown12: Option<u32>,
    pub unknown13: Option<u32>,
    pub unknown14: Option<u32>,
    pub unknown15: Option<f32>,
}

impl AtpEntryData41 {
    pub fn read(cursor: &mut Cursor<&[u8]>, length: u32) -> anyhow::Result<Self> {
        let mut data = AtpEntryData41 {
            unknown1: None,
            unknown2: None,
            file: None,
            sub_files: None,
            unknown5: None,
            unknown6: None,
            unknown7: None,
            unknown8: None,
            unknown9: None,
            unknown10: None,
            unknown11: None,
            unknown12: None,
            unknown13: None,
            unknown14: None,
            unknown15: None,
        };

        let mut _item_length = 0;
        read_data!(
            "AtpEntryData41",
            cursor,
            length,
            _item_length,
            1 => data.unknown1 = Some([cursor.read_f32_le()?, cursor.read_f32_le()?, cursor.read_f32_le()?]),
            2 => data.unknown2 = Some(cursor.read_u32_le()?),
            3 => data.file = Some(AtpEntryData41File::read(cursor, _item_length - 2)?),
            4 => data.sub_files = Some(read_sub_files(cursor, _item_length - 2)?),
            5 => data.unknown5 = Some(cursor.read_u32_le()?),
            6 => data.unknown6 = Some(cursor.read_u8_vec((_item_length - 2) as usize)?),
            7 => data.unknown7 = Some(cursor.read_u32_le()?),
            8 => data.unknown8 = Some(cursor.read_u64_le()?),
            9 => data.unknown9 = Some(cursor.read_u32_le()?),
            10 => data.unknown10 = Some(cursor.read_u32_le()?),
            11 => data.unknown11 = Some(cursor.read_u32_le()?),
            12 => data.unknown12 = Some(cursor.read_u32_le()?),
            13 => data.unknown13 = Some(cursor.read_u32_le()?),
            14 => data.unknown14 = Some(cursor.read_u32_le()?),
            15 => data.unknown15 = Some(cursor.read_f32_le()?),
        );

        Ok(data)
    }
}

fn read_sub_files(
    cursor: &mut Cursor<&[u8]>,
    length: u32,
) -> anyhow::Result<Vec<AtpEntryData41SubFile>> {
    let mut sub_files = vec![];
    let mut cur_length = 0;
    while cur_length < length {
        let item_length = cursor.read_u32_le()?;
        sub_files.push(AtpEntryData41SubFile::read(cursor, item_length)?);
        cur_length += item_length + 4;
    }

    Ok(sub_files)
}

#[derive(Debug, Serialize)]
pub struct AtpEntryData41File {
    pub path: Option<SizedBig5String>,
    pub unknown2: Option<u32>,
    pub next_name: Option<AtpEntryData41FileNextName>,
    pub unknown4: Option<Vec<u8>>,
    pub unknown5: Option<[f32; 2]>,
    pub names: Option<Vec<SizedBig5String>>,
}

impl AtpEntryData41File {
    pub fn read(cursor: &mut Cursor<&[u8]>, length: u32) -> anyhow::Result<Self> {
        let mut data = AtpEntryData41File {
            path: None,
            unknown2: None,
            next_name: None,
            unknown4: None,
            unknown5: None,
            names: None,
        };

        let mut _item_length = 0;
        read_data!(
            "AtpEntryData41File",
            cursor,
            length,
            _item_length,
            1 => data.path = Some(SizedBig5String::read(cursor)?),
            2 => data.unknown2 = Some(cursor.read_u32_le()?),
            3 => data.next_name = Some(AtpEntryData41FileNextName::read(cursor)?),
            4 => data.unknown4 = Some(cursor.read_u8_vec(10)?),
            5 => data.unknown5 = Some([cursor.read_f32_le()?, cursor.read_f32_le()?]),
            6 => data.names = Some(read_names(cursor)?),
        );

        Ok(data)
    }
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct AtpEntryData41FileNextName {
    pub unknown1: u32,
    pub name: SizedBig5String,
    pub unknown3: u16,
}

fn read_names(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Vec<SizedBig5String>> {
    let mut names = vec![];
    let count = cursor.read_u32_le()?;
    for _ in 0..count {
        let name = SizedBig5String::read(cursor)?;
        names.push(name);
    }

    Ok(names)
}

#[derive(Debug, Serialize)]
pub struct AtpEntryData41SubFile {
    pub name: Option<SizedBig5String>,
    pub path: Option<SizedBig5String>,
    pub unknown3: Option<u32>,
    pub unknown4: Option<Vec<u8>>,
    pub unknown5: Option<u32>,
    pub unknown6: Option<u32>,
}

impl AtpEntryData41SubFile {
    pub fn read(cursor: &mut Cursor<&[u8]>, length: u32) -> anyhow::Result<Self> {
        let mut data = AtpEntryData41SubFile {
            name: None,
            path: None,
            unknown3: None,
            unknown4: None,
            unknown5: None,
            unknown6: None,
        };

        let mut _item_length = 0;
        read_data!(
            "AtpEntryData41SubFile",
            cursor,
            length,
            _item_length,
            1 => data.name = Some(SizedBig5String::read(cursor)?),
            2 => data.path = Some(SizedBig5String::read(cursor)?),
            3 => data.unknown3 = Some(cursor.read_u32_le()?),
            4 => data.unknown4 = Some(cursor.read_u8_vec((_item_length - 2) as usize)?),
            5 => data.unknown5 = Some(cursor.read_u32_le()?),
            6 => data.unknown6 = Some(cursor.read_u32_le()?),
        );

        Ok(data)
    }
}

#[derive(Debug, Serialize)]
pub struct AtpEntryData42 {
    pub unknown1: Option<AtpEntryData42Unknown>,
    pub unknown2: Option<f32>,
    pub unknown3: Option<u32>,
    pub unknown4: Option<u32>,
    pub unknown5: Option<u64>,
    pub unknown6: Option<u32>,
    pub unknown7: Option<[u32; 3]>,
    pub unknown8: Option<u32>,
    pub unknown9: Option<u32>,
    pub unknown10: Option<Vec<u8>>,
    pub unknown11: Option<u32>,
    pub unknown12: Option<Vec<u8>>,
    pub unknown13: Option<f32>,
    pub unknown14: Option<f32>,
}

impl AtpEntryData42 {
    pub fn read(cursor: &mut Cursor<&[u8]>, length: u32) -> anyhow::Result<Self> {
        let mut data = AtpEntryData42 {
            unknown1: None,
            unknown2: None,
            unknown3: None,
            unknown4: None,
            unknown5: None,
            unknown6: None,
            unknown7: None,
            unknown8: None,
            unknown9: None,
            unknown10: None,
            unknown11: None,
            unknown12: None,
            unknown13: None,
            unknown14: None,
        };

        let mut _item_length = 0;
        read_data!(
            "AtpEntryData42",
            cursor,
            length,
            _item_length,
            1 => data.unknown1 = Some(AtpEntryData42Unknown::read(cursor, _item_length - 2)?),
            2 => data.unknown2 = Some(cursor.read_f32_le()?),
            3 => data.unknown3 = Some(cursor.read_u32_le()?),
            4 => data.unknown4 = Some(cursor.read_u32_le()?),
            5 => data.unknown5 = Some(cursor.read_u64_le()?),
            6 => data.unknown6 = Some(cursor.read_u32_le()?),
            7 => data.unknown7 = Some([cursor.read_u32_le()?, cursor.read_u32_le()?, cursor.read_u32_le()?]),
            8 => data.unknown8 = Some(cursor.read_u32_le()?),
            9 => data.unknown9 = Some(cursor.read_u32_le()?),
            10 => data.unknown10 = Some(cursor.read_u8_vec((_item_length - 2) as usize)?),
            11 => data.unknown11 = Some(cursor.read_u32_le()?),
            12 => data.unknown12 = Some(cursor.read_u8_vec((_item_length - 2) as usize)?),
            13 => data.unknown13 = Some(cursor.read_f32_le()?),
            14 => data.unknown14 = Some(cursor.read_f32_le()?),
        );

        Ok(data)
    }
}

#[derive(Debug, Serialize)]
pub struct AtpEntryData42Unknown {
    pub path: Option<SizedBig5String>,
    pub unknown2: Option<u32>,
    pub unknown4: Option<Vec<u8>>, // 10 bytes
    pub unknown5: Option<[f32; 2]>,
}

impl AtpEntryData42Unknown {
    pub fn read(cursor: &mut Cursor<&[u8]>, length: u32) -> anyhow::Result<Self> {
        let mut data = AtpEntryData42Unknown {
            path: None,
            unknown2: None,
            unknown4: None,
            unknown5: None,
        };

        let mut _item_length = 0;
        read_data!(
            "AtpEntryData42Unknown",
            cursor,
            length,
            _item_length,
            1 => data.path = Some(SizedBig5String::read(cursor)?),
            2 => data.unknown2 = Some(cursor.read_u32_le()?),
            4 => data.unknown4 = Some(cursor.read_u8_vec(10)?),
            5 => data.unknown5 = Some([cursor.read_f32_le()?, cursor.read_f32_le()?]),
        );

        Ok(data)
    }
}

#[derive(Debug, Serialize)]
pub struct AtpEntryData45 {
    pub unknown3: Option<Vec<AtpEntryData45Unknown>>,
    pub unknown4: Option<u32>,
    pub unknown5: Option<u32>,
    pub unknown6: Option<u32>,
    pub unknown7: Option<u32>,
    pub unknown8: Option<u32>,
    pub unknown9: Option<Vec<AtpEntryFilePath>>,
    pub unknown10: Option<[f32; 2]>,
}

impl AtpEntryData45 {
    pub fn read(cursor: &mut Cursor<&[u8]>, length: u32) -> anyhow::Result<Self> {
        let mut data = AtpEntryData45 {
            unknown3: None,
            unknown4: None,
            unknown5: None,
            unknown6: None,
            unknown7: None,
            unknown8: None,
            unknown9: None,
            unknown10: None,
        };

        let mut _item_length = 0;
        read_data!(
            "AtpEntryData45",
            cursor,
            length,
            _item_length,
            3 => data.unknown3 = Some(read_data45_unknown(cursor)?),
            4 => data.unknown4 = Some(cursor.read_u32_le()?),
            5 => data.unknown5 = Some(cursor.read_u32_le()?),
            6 => data.unknown6 = Some(cursor.read_u32_le()?),
            7 => data.unknown7 = Some(cursor.read_u32_le()?),
            8 => data.unknown8 = Some(cursor.read_u32_le()?),
            9 => data.unknown9 = Some(read_file_path(cursor)?),
            10 => data.unknown10 = Some([cursor.read_f32_le()?, cursor.read_f32_le()?]),
        );

        Ok(data)
    }
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct AtpEntryData45Unknown {
    pub dword: [u32; 3],
    pub float: [f32; 2],
}

fn read_data45_unknown(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Vec<AtpEntryData45Unknown>> {
    let mut unknowns = vec![];
    let count = cursor.read_u32_le()?;
    for _ in 0..count {
        let unknown = AtpEntryData45Unknown::read(cursor)?;
        unknowns.push(unknown);
    }

    Ok(unknowns)
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct AtpEntryFilePath {
    pub path: SizedBig5String,
    pub unknown_float: f32,
}

fn read_file_path(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Vec<AtpEntryFilePath>> {
    let mut paths = vec![];
    let count = cursor.read_u32_le()?;
    for _ in 0..count {
        let path = AtpEntryFilePath::read(cursor)?;
        paths.push(path);
    }

    Ok(paths)
}
