use std::{
    collections::HashMap,
    io::{Cursor, Write},
};

use binrw::{binrw, BinRead, BinWrite};
use common::{SeekRead, SeekWrite};

use crate::memory_file::MemoryFile;

pub struct YpkArchive {
    reader: Box<dyn SeekRead>,
    pub entries: Vec<YpkEntry>,
    entries_hash: HashMap<u64, Vec<usize>>,
}

impl YpkArchive {
    pub fn load(mut reader: Box<dyn SeekRead>) -> anyhow::Result<Self> {
        let header = YpkHeader::read(&mut reader)?;

        reader.seek(std::io::SeekFrom::Start(header.entry_offset))?;
        let mut entries = Vec::with_capacity(header.entry_count as usize);
        for _ in 0..header.entry_count {
            entries.push(YpkEntry::read(&mut reader)?);
        }

        let mut entries_hash = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            entries_hash
                .entry(entry.hash)
                .or_insert_with(Vec::new)
                .push(i);
        }

        Ok(Self {
            reader,
            entries,
            entries_hash,
        })
    }

    pub fn open(&mut self, name: &str) -> std::io::Result<MemoryFile> {
        let (offset, actual_size) = {
            let entry = self.get_entry(name).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Entry {:?} not found", name),
                )
            })?;

            (entry.offset, entry.actual_size)
        };

        self.reader.seek(std::io::SeekFrom::Start(offset))?;
        let mut buf = vec![0; actual_size as usize];
        self.reader.read_exact(&mut buf)?;

        let buf = zstd::stream::decode_all::<&[u8]>(buf.as_ref());
        Ok(MemoryFile::new(Cursor::new(buf.unwrap())))
    }

    fn get_entry(&self, name: &str) -> Option<&YpkEntry> {
        let name = normorlize_path(name);
        let name_for_hash = name.to_lowercase();

        let hash = xxhash_rust::xxh3::xxh3_64(name_for_hash.as_bytes());
        self.entries_hash.get(&hash).and_then(|indices| {
            if indices.len() == 1 {
                Some(&self.entries[indices[0]])
            } else {
                indices
                    .iter()
                    .find(|&&i| {
                        std::str::from_utf8(&self.entries[i].name)
                            .unwrap()
                            .to_lowercase()
                            == name
                    })
                    .map(|&i| &self.entries[i])
            }
        })
    }
}

#[derive(Debug)]
#[binrw]
#[brw(little)]
#[brw(magic = b"YPK\x01")]
struct YpkHeader {
    entry_count: u32,
    entry_offset: u64,
}

#[derive(Debug)]
#[binrw]
#[brw(little)]
pub struct YpkEntry {
    hash: u64,
    name_len: u32,

    #[br(count = name_len)]
    name: Vec<u8>,

    offset: u64,
    original_size: u32,
    actual_size: u32,
}

pub struct YpkWriter {
    writer: Box<dyn SeekWrite>,
    entries: Vec<YpkEntry>,
}

impl YpkWriter {
    pub fn new(mut writer: Box<dyn SeekWrite>) -> anyhow::Result<Self> {
        YpkHeader {
            entry_count: 0,
            entry_offset: 0,
        }
        .write(&mut writer)?;

        Ok(Self {
            writer,
            entries: vec![],
        })
    }

    pub fn write_file(&mut self, name: &str, data: &[u8]) -> std::io::Result<()> {
        let offset = self.writer.stream_position()?;
        let original_size = data.len() as u32;

        let name = normorlize_path(name);
        let name_for_hash = name.to_lowercase();
        let name = name.as_bytes();

        let data = zstd::stream::encode_all(data, 0)?;

        self.entries.push(YpkEntry {
            hash: xxhash_rust::xxh3::xxh3_64(name_for_hash.as_bytes()),
            name_len: name.len() as u32,
            name: name.to_vec(),
            offset,
            original_size,
            actual_size: data.len() as u32,
        });

        self.writer.write_all(&data)?;
        Ok(())
    }

    pub fn finish(mut self) -> anyhow::Result<Box<dyn SeekWrite>> {
        let entry_offset = self.writer.stream_position()?;
        let entry_count = self.entries.len() as u32;

        for entry in self.entries {
            entry.write(&mut self.writer)?;
        }

        self.writer.rewind()?;
        YpkHeader {
            entry_count,
            entry_offset,
        }
        .write(&mut self.writer)?;

        Ok(self.writer)
    }
}

fn normorlize_path(path: &str) -> String {
    path.replace("\\", "/")
        .chars()
        .skip_while(|&c| c == '/' || c == '.')
        .collect()
}
