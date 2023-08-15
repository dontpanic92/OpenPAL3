use std::{
    collections::HashMap,
    io::{Cursor, SeekFrom},
    path::Path,
};

use byteorder::{LittleEndian, WriteBytesExt};
use common::read_ext::ReadExt;

use crate::fs::{memory_file::MemoryFile, plain_fs::PlainArchive, SeekRead};

pub struct ImdArchive {
    reader: Box<dyn SeekRead>,
    _meta: ImdMeta,
    pub files: HashMap<String, ImdFile>,
}

impl ImdArchive {
    pub fn load(mut reader: Box<dyn SeekRead>) -> anyhow::Result<ImdArchive> {
        let meta = ImdMeta::read(&mut reader)?;
        let mut files = HashMap::new();

        reader.seek(SeekFrom::Start(4))?;
        for _ in 0..meta.file_count {
            let file = ImdFile::read(&mut reader)?;
            files.insert(file.name.clone(), file);
        }

        Ok(Self {
            reader,
            _meta: meta,
            files,
        })
    }
}

impl PlainArchive for ImdArchive {
    fn open<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<MemoryFile> {
        let path = path.as_ref().to_str().unwrap();

        if let Some(file) = self.files.get(path) {
            self.reader
                .seek(SeekFrom::Start(file.start_position as u64))?;
            let data = self.reader.read_u8_vec(file.file_size as usize)?;

            let data = match file.file_type {
                3 => {
                    let mut output = vec![];
                    output.write_u32::<LittleEndian>(file.width)?;
                    output.write_u32::<LittleEndian>(file.height)?;
                    let mut data = decode_03(&data)?;
                    output.append(&mut data);
                    output
                }
                5 => {
                    let mut output = vec![];
                    output.write_u32::<LittleEndian>(file.width)?;
                    output.write_u32::<LittleEndian>(file.height)?;
                    let mut data = decode_05(&data)?;
                    output.append(&mut data);
                    output
                }
                7 | 9 => data,
                i => unimplemented!("Unsupported imd file type: {}", i),
            };

            Ok(MemoryFile::new(Cursor::new(data)))
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::NotFound))?
        }
    }

    fn files(&self) -> Vec<String> {
        self.files.keys().map(|s| s.to_string()).collect()
    }
}

fn decode_03(mut data: &[u8]) -> anyhow::Result<Vec<u8>> {
    let data = data.read_w_vec(data.len() / 2)?;
    let mut output = vec![];
    let mut i = 0;
    while i < data.len() {
        let len = (data[i] & 0x7fff) + 1;
        let ty = data[i] & 0x8000;
        i += 1;
        if ty == 0 {
            for j in 0..len as usize {
                let pixel = transform_rgb565(data[i + j]);
                output.append(&mut pixel.to_vec());
            }

            i += len as usize;
        } else {
            let pixel = transform_rgb565(data[i]);
            for _ in 0..len as usize {
                output.append(&mut pixel.to_vec());
            }

            i += 1;
        }
    }

    Ok(output)
}

fn decode_05(mut data: &[u8]) -> anyhow::Result<Vec<u8>> {
    let data = data.read_dw_vec(data.len() / 4)?;
    let mut output = vec![];
    let mut i = 0;
    while i < data.len() {
        let len = (data[i] & 0x7fffffff) + 1;
        let ty = data[i] & 0x80000000;
        i += 1;
        if ty == 0 {
            for j in 0..len as usize {
                let mut v = vec![];
                v.write_u32::<LittleEndian>(data[i + j])?;
                v.swap(0, 2);
                output.append(&mut v);
            }

            i += len as usize;
        } else {
            let mut v = vec![];
            v.write_u32::<LittleEndian>(data[i])?;
            v.swap(0, 2);
            for _ in 0..len as usize {
                output.append(&mut v.clone());
            }

            i += 1;
        }
    }

    Ok(output)
}

fn transform_rgb565(short: u16) -> [u8; 4] {
    let b = (((short & 0x1f) as u32 * 527 + 23) >> 6) as u8;
    let g = ((((short & 0x7e0) >> 5) as u32 * 259 + 23) >> 6) as u8;
    let r = ((((short >> 11) & 0x1f) as u32 * 527 + 23) >> 6) as u8;
    [r, g, b, 255]
}

#[derive(thiserror::Error, Debug)]
pub enum ImdReadError {
    #[error("Incorrect magic found for fmb. Should be: 0x464d4220, found {0}")]
    IncorrectMagic(u32),
}

#[derive(Debug, Clone)]
pub struct ImdMeta {
    pub magic: u32,
    pub file_count: u32,
}

impl ImdMeta {
    pub fn read(reader: &mut dyn SeekRead) -> anyhow::Result<Self> {
        reader.seek(SeekFrom::Start(0))?;

        let magic = reader.read_u32_le()?;
        if magic != 0x20444d49 {
            Err(ImdReadError::IncorrectMagic(magic))?;
        }

        reader.seek(SeekFrom::End(-4))?;
        let file_count = reader.read_u32_le()?;
        Ok(Self { magic, file_count })
    }
}

#[derive(Debug, Clone)]
pub struct ImdFile {
    pub file_size: u32,
    pub file_type: u32,
    pub width: u32,
    pub height: u32,
    pub name: String,
    pub start_position: u64,
    pub unknown12: Vec<u32>,
}

impl ImdFile {
    pub fn read(reader: &mut dyn SeekRead) -> anyhow::Result<Self> {
        let current_position = reader.stream_position().unwrap();

        let chunk_size = reader.read_u32_le()?;
        let unknown12 = reader.read_dw_vec(12)?;

        let name_length = reader.read_u32_le()?;
        let name = reader.read_gbk_string(name_length as usize)?;
        let file_type = unknown12[1];
        let width = unknown12[4];
        let height = unknown12[5];
        let file_size = unknown12[8];
        let start_position = reader.stream_position()?;

        reader.seek(SeekFrom::Start(current_position + chunk_size as u64))?;

        Ok(Self {
            file_size,
            file_type,
            width,
            height,
            name,
            start_position,
            unknown12,
        })
    }
}
