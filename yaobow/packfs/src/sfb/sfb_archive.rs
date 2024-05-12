use std::{
    collections::HashMap,
    io::{Cursor, SeekFrom},
    path::Path,
};

use common::read_ext::ReadExt;
use common::SeekRead;

use crate::{memory_file::MemoryFile, plain_fs::PlainArchive};

pub struct SfbArchive {
    reader: Box<dyn SeekRead>,
    _meta: SfbMeta,
    pub files: HashMap<String, SfbFile>,
}

impl SfbArchive {
    pub fn load(mut reader: Box<dyn SeekRead>) -> anyhow::Result<SfbArchive> {
        let meta = SfbMeta::read(&mut reader)?;
        let mut files = HashMap::new();

        for (i, entry) in meta.entries.iter().enumerate() {
            reader.seek(SeekFrom::Start(*entry as u64))?;
            let file = SfbFile::read(&mut reader)?;
            files.insert(format!("{}.mp3", i), file);
        }

        Ok(Self {
            reader,
            _meta: meta,
            files,
        })
    }
}

impl PlainArchive for SfbArchive {
    fn open<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<MemoryFile> {
        let path = path.as_ref().to_str().unwrap();

        if let Some(file) = self.files.get(path) {
            self.reader
                .seek(SeekFrom::Start(file.start_position as u64))?;
            let data = self.reader.read_u8_vec(file.file_size as usize)?;

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
pub enum SfbReadError {
    #[error("Incorrect magic found for Sfb. Should be: 0x464d4220, found {0}")]
    IncorrectMagic(u32),
}

#[derive(Debug, Clone)]
pub struct SfbMeta {
    pub entry_count: u32,
    pub entries: Vec<u32>,
}

impl SfbMeta {
    pub fn read(reader: &mut dyn SeekRead) -> anyhow::Result<Self> {
        reader.seek(SeekFrom::Start(0))?;
        let meta_position = reader.read_u32_le()?;

        reader.seek(SeekFrom::Start(meta_position as u64))?;
        let entry_count = reader.read_u32_le()?;
        let entry_table_start = reader.read_u32_le()?;

        reader.seek(SeekFrom::Start(entry_table_start as u64))?;
        let entries = reader.read_dw_vec(entry_count as usize)?;
        Ok(Self {
            entry_count,
            entries,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SfbFile {
    pub file_size: u32,
    pub start_position: u64,
}

impl SfbFile {
    pub fn read(reader: &mut dyn SeekRead) -> anyhow::Result<Self> {
        let file_size = reader.read_u32_le()?;
        let start_position = reader.stream_position().unwrap();

        Ok(Self {
            file_size,
            start_position,
        })
    }
}
