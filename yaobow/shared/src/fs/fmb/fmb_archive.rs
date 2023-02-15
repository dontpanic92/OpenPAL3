use std::{
    collections::HashMap,
    io::{Cursor, SeekFrom},
    path::Path,
};

use common::read_ext::ReadExt;

use crate::fs::{memory_file::MemoryFile, plain_fs::PlainArchive, SeekRead};

#[derive(Debug)]
pub struct FmbArchive<T: AsRef<[u8]>> {
    cursor: Cursor<T>,
    _meta: FmbMeta,
    pub files: HashMap<String, FmbFile>,
}

impl<T: AsRef<[u8]>> FmbArchive<T> {
    pub fn load(mut cursor: Cursor<T>) -> anyhow::Result<FmbArchive<T>> {
        let meta = FmbMeta::read(&mut cursor)?;
        let mut files = HashMap::new();

        cursor.set_position(4);
        for _ in 0..meta.file_count {
            let file = FmbFile::read(&mut cursor)?;
            files.insert(file.name.clone(), file);
        }

        Ok(Self {
            cursor,
            _meta: meta,
            files,
        })
    }
}

impl<T: AsRef<[u8]>> PlainArchive for FmbArchive<T> {
    fn open<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<MemoryFile> {
        let path = path.as_ref().to_str().unwrap();

        if let Some(file) = self.files.get(path) {
            self.cursor.set_position(file.start_position as u64);
            let mut data = self.cursor.read_u8_vec(file.compressed_size as usize)?;

            if file.is_compressed == 1 {
                let lzo = minilzo_rs::LZO::init().unwrap();
                let decompressed_content = lzo
                    .decompress(&data, file.uncompressed_size as usize)
                    .or_else(|e| {
                    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                })?;

                data = decompressed_content;
            }

            Ok(MemoryFile::new(Cursor::new(data)))
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::NotFound))?
        }
    }

    fn files(&self) -> Vec<String> {
        self.files.keys().map(|s| s.to_string()).collect()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum FmbReadError {
    #[error("Incorrect magic found for fmb. Should be: 0x464d4220, found {0}")]
    IncorrectMagic(u32),
}

#[derive(Debug, Clone)]
pub struct FmbMeta {
    pub magic: u32,
    pub file_count: u32,
}

impl FmbMeta {
    pub fn read(reader: &mut dyn SeekRead) -> anyhow::Result<Self> {
        reader.seek(SeekFrom::Start(0))?;

        let magic = reader.read_u32_le()?;
        if magic != 0x20424d46 {
            Err(FmbReadError::IncorrectMagic(magic))?;
        }

        reader.seek(SeekFrom::End(-4))?;
        let file_count = reader.read_u32_le()?;
        Ok(Self { magic, file_count })
    }
}

#[derive(Debug, Clone)]
pub struct FmbFile {
    pub name: String,
    pub start_position: u64,
    pub is_compressed: u32,
    pub uncompressed_size: u32,
    pub compressed_size: u32,
    pub unknown4: u32,
    pub unknown5: u32,
}

impl FmbFile {
    pub fn read(reader: &mut dyn SeekRead) -> anyhow::Result<Self> {
        let current_position = reader.stream_position().unwrap();

        let chunk_size = reader.read_u32_le()?;
        let is_compressed = reader.read_u32_le()?;
        let uncompressed_size = reader.read_u32_le()?;
        let compressed_size = reader.read_u32_le()?;
        let unknown4 = reader.read_u32_le()?;
        let unknown5 = reader.read_u32_le()?;

        let name_length = reader.read_u32_le()?;
        let name = reader.read_gbk_string(name_length as usize)?;
        let start_position = reader.stream_position()?;

        reader.seek(SeekFrom::Start(current_position + chunk_size as u64))?;

        Ok(Self {
            name,
            start_position,
            is_compressed,
            uncompressed_size,
            compressed_size,
            unknown4,
            unknown5,
        })
    }
}
