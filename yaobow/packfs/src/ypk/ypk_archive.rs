use std::{
    collections::HashMap,
    io::{Cursor, Read, Write},
    ops::DerefMut,
    sync::{Arc, Mutex},
};

use binrw::{binrw, BinRead, BinWrite};
use common::{SeekRead, SeekWrite};
use mini_fs::File;

use crate::{memory_file::MemoryFile, streaming_file::StreamingFile};

pub struct YpkArchive {
    reader: Arc<Mutex<dyn SeekRead + Send + Sync>>,
    pub entries: Vec<YpkEntry>,
    entries_hash: HashMap<u64, Vec<usize>>,
}

impl YpkArchive {
    pub fn load(file: Arc<Mutex<dyn SeekRead + Send + Sync>>) -> anyhow::Result<Self> {
        let mut reader = file.lock().unwrap();
        let header = YpkHeader::read(&mut reader.deref_mut())?;

        let file_end = reader.seek(std::io::SeekFrom::End(0))?;
        let entry_list_size = file_end - header.entry_offset;

        reader.seek(std::io::SeekFrom::Start(header.entry_offset))?;
        let mut entry_list = Vec::with_capacity(entry_list_size as usize);
        reader.read_to_end(&mut entry_list);
        let mut entry_reader = Cursor::new(entry_list);

        let mut entries = Vec::with_capacity(header.entry_count as usize);
        for _ in 0..header.entry_count {
            entries.push(YpkEntry::read(&mut entry_reader)?);
        }

        let mut entries_hash = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            entries_hash
                .entry(entry.hash)
                .or_insert_with(Vec::new)
                .push(i);
        }

        drop(reader);

        Ok(Self {
            reader: file,
            entries,
            entries_hash,
        })
    }

    pub fn open(&mut self, name: &str) -> std::io::Result<File> {
        let (offset, actual_size, is_compressed) = {
            let entry = self.get_entry(name).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Entry {:?} not found", name),
                )
            })?;

            (entry.offset, entry.actual_size, entry.is_compressed == 1)
        };

        let mut streaming =
            StreamingFile::new(self.reader.clone(), offset, offset + actual_size as u64);

        if is_compressed {
            let buf = zstd::stream::decode_all(&mut streaming)?;
            return Ok(MemoryFile::new(Cursor::new(buf)).into());
        } else {
            return Ok(streaming.into());
        }
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
    is_compressed: u32,
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

        let (is_compressed, data) = if name.ends_with(".bik") {
            (false, data.to_vec())
        } else {
            (true, zstd::stream::encode_all(data, 0)?)
        };

        let name = normorlize_path(name);
        let name_for_hash = name.to_lowercase();
        let name = name.as_bytes();

        self.entries.push(YpkEntry {
            hash: xxhash_rust::xxh3::xxh3_64(name_for_hash.as_bytes()),
            name_len: name.len() as u32,
            name: name.to_vec(),
            offset,
            is_compressed: is_compressed as u32,
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
