use std::{
    collections::HashMap,
    io::{Cursor, Read, Seek, Write},
    ops::DerefMut,
    sync::{Arc, Mutex},
};

use binrw::{BinRead, BinWrite};
use mini_fs::File;

use crate::file::{MemoryFile, StreamingFile};

const HEADER_SIZE: u64 = 16;
const MIN_ENTRY_SIZE: u64 = 32;
const MAX_ENTRY_COUNT: u32 = 1_000_000;
const MAX_INDEX_SIZE: u64 = 256 * 1024 * 1024;
const MAX_DECOMPRESSED_ENTRY_SIZE: u32 = 1024 * 1024 * 1024;

pub trait SeekRead: Read + Seek {}
impl<T> SeekRead for T where T: Read + Seek {}

pub trait SeekWrite: Write + Seek {}
impl<T> SeekWrite for T where T: Write + Seek {}

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
        if header.entry_offset < HEADER_SIZE || header.entry_offset > file_end {
            anyhow::bail!("YPK entry table offset is outside the archive");
        }
        let entry_list_size = file_end
            .checked_sub(header.entry_offset)
            .ok_or_else(|| anyhow::anyhow!("invalid YPK entry table range"))?;
        if entry_list_size > MAX_INDEX_SIZE {
            anyhow::bail!("YPK entry table exceeds the supported size");
        }
        if header.entry_count > MAX_ENTRY_COUNT
            || u64::from(header.entry_count) > entry_list_size / MIN_ENTRY_SIZE
        {
            anyhow::bail!("YPK entry count does not fit the entry table");
        }
        reader.seek(std::io::SeekFrom::Start(header.entry_offset))?;
        let entry_list_capacity = usize::try_from(entry_list_size)
            .map_err(|_| anyhow::anyhow!("YPK entry table is too large for this platform"))?;
        let mut entry_list = Vec::with_capacity(entry_list_capacity);
        reader.read_to_end(&mut entry_list)?;
        let mut entry_reader = Cursor::new(entry_list);

        let mut entries = Vec::with_capacity(header.entry_count as usize);
        for _ in 0..header.entry_count {
            let entry = read_entry(&mut entry_reader)?;
            if entry.is_compressed > 1 {
                anyhow::bail!("YPK entry has an invalid compression flag");
            }
            let entry_name = std::str::from_utf8(&entry.name)?;
            let expected_hash =
                xxhash_rust::xxh3::xxh3_64(normalize_path(entry_name).to_lowercase().as_bytes());
            if entry.hash != expected_hash {
                anyhow::bail!("YPK entry hash does not match its name");
            }
            if entry.is_compressed == 0 && entry.actual_size != entry.original_size {
                anyhow::bail!("uncompressed YPK entry has inconsistent sizes");
            }
            if entry.is_compressed == 1 && entry.original_size > MAX_DECOMPRESSED_ENTRY_SIZE {
                anyhow::bail!("compressed YPK entry exceeds the supported output size");
            }
            let entry_end = entry
                .offset
                .checked_add(u64::from(entry.actual_size))
                .ok_or_else(|| anyhow::anyhow!("YPK entry extent overflows"))?;
            if entry.offset < HEADER_SIZE || entry_end > header.entry_offset {
                anyhow::bail!("YPK entry data is outside the archive data region");
            }
            entries.push(entry);
        }
        if entry_reader.position() != entry_reader.get_ref().len() as u64 {
            anyhow::bail!("YPK entry table contains trailing data");
        }

        let mut entries_hash = HashMap::new();
        for (index, entry) in entries.iter().enumerate() {
            entries_hash
                .entry(entry.hash)
                .or_insert_with(Vec::new)
                .push(index);
        }
        drop(reader);

        Ok(Self {
            reader: file,
            entries,
            entries_hash,
        })
    }

    pub fn open(&mut self, name: &str) -> std::io::Result<File> {
        let (offset, actual_size, original_size, is_compressed) = {
            let entry = self.get_entry(name).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("entry {name:?} not found"),
                )
            })?;
            (
                entry.offset,
                entry.actual_size,
                entry.original_size,
                entry.is_compressed == 1,
            )
        };

        let end = offset.checked_add(u64::from(actual_size)).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "entry overflow")
        })?;
        let streaming = StreamingFile::new(self.reader.clone(), offset, end);
        if is_compressed {
            let decoder = zstd::stream::read::Decoder::new(streaming)?;
            let mut buffer = Vec::with_capacity(original_size as usize);
            decoder
                .take(u64::from(original_size) + 1)
                .read_to_end(&mut buffer)?;
            if buffer.len() != original_size as usize {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "decompressed YPK entry size does not match its header",
                ));
            }
            Ok(MemoryFile::new(Cursor::new(buffer)).into())
        } else {
            Ok(streaming.into())
        }
    }

    fn get_entry(&self, name: &str) -> Option<&YpkEntry> {
        let name = normalize_path(name);
        let lower_name = name.to_lowercase();
        let hash = xxhash_rust::xxh3::xxh3_64(lower_name.as_bytes());
        self.entries_hash.get(&hash).and_then(|indices| {
            indices
                .iter()
                .find(|&&index| {
                    normalize_path(std::str::from_utf8(&self.entries[index].name).unwrap())
                        .to_lowercase()
                        == lower_name
                })
                .map(|&index| &self.entries[index])
        })
    }
}

#[derive(Debug, BinRead, BinWrite)]
#[brw(little)]
#[brw(magic = b"YPK\x01")]
struct YpkHeader {
    entry_count: u32,
    entry_offset: u64,
}

#[derive(Debug, BinWrite)]
#[bw(little)]
pub struct YpkEntry {
    hash: u64,
    name_len: u32,
    name: Vec<u8>,
    offset: u64,
    is_compressed: u32,
    original_size: u32,
    actual_size: u32,
}

impl YpkEntry {
    pub fn name(&self) -> &str {
        std::str::from_utf8(&self.name).unwrap_or("")
    }
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
            entries: Vec::new(),
        })
    }

    pub fn write_file(&mut self, name: &str, data: &[u8]) -> std::io::Result<()> {
        let offset = self.writer.stream_position()?;
        let original_size = u32::try_from(data.len()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "YPK entry exceeds the format's 32-bit size limit",
            )
        })?;
        let (is_compressed, data) = if name.ends_with(".bik") {
            (false, data.to_vec())
        } else {
            (true, zstd::stream::encode_all(data, 0)?)
        };

        let name = normalize_path(name);
        let lower_name = name.to_lowercase();
        let name = name.as_bytes();
        let name_len = u32::try_from(name.len()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "YPK path is too long")
        })?;
        let actual_size = u32::try_from(data.len()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "compressed YPK entry exceeds the format's 32-bit size limit",
            )
        })?;
        self.entries.push(YpkEntry {
            hash: xxhash_rust::xxh3::xxh3_64(lower_name.as_bytes()),
            name_len,
            name: name.to_vec(),
            offset,
            is_compressed: is_compressed as u32,
            original_size,
            actual_size,
        });
        self.writer.write_all(&data)
    }

    pub fn finish(mut self) -> anyhow::Result<Box<dyn SeekWrite>> {
        let entry_offset = self.writer.stream_position()?;
        let entry_count = u32::try_from(self.entries.len())
            .map_err(|_| anyhow::anyhow!("YPK contains too many entries"))?;
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

fn read_entry(reader: &mut Cursor<Vec<u8>>) -> anyhow::Result<YpkEntry> {
    let hash = read_u64(reader)?;
    let name_len = read_u32(reader)?;
    let remaining = reader
        .get_ref()
        .len()
        .checked_sub(reader.position() as usize)
        .ok_or_else(|| anyhow::anyhow!("invalid YPK entry cursor"))?;
    let name_len_usize =
        usize::try_from(name_len).map_err(|_| anyhow::anyhow!("YPK path is too long"))?;
    if name_len_usize > remaining.saturating_sub(20) {
        anyhow::bail!("YPK entry name length exceeds the entry table");
    }
    let mut name = vec![0; name_len_usize];
    reader.read_exact(&mut name)?;
    if name.is_empty() || std::str::from_utf8(&name).is_err() {
        anyhow::bail!("YPK entry name is empty or invalid UTF-8");
    }
    Ok(YpkEntry {
        hash,
        name_len,
        name,
        offset: read_u64(reader)?,
        is_compressed: read_u32(reader)?,
        original_size: read_u32(reader)?,
        actual_size: read_u32(reader)?,
    })
}

fn read_u32(reader: &mut impl Read) -> std::io::Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> std::io::Result<u64> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .chars()
        .skip_while(|&character| character == '/' || character == '.')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_bytes(bytes: Vec<u8>) -> anyhow::Result<YpkArchive> {
        YpkArchive::load(Arc::new(Mutex::new(Cursor::new(bytes))))
    }

    #[test]
    fn rejects_entry_table_outside_file() {
        let mut bytes = b"YPK\x01".to_vec();
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&100_u64.to_le_bytes());
        assert!(load_bytes(bytes).is_err());
    }

    #[test]
    fn rejects_name_length_larger_than_index() {
        let mut bytes = b"YPK\x01".to_vec();
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&HEADER_SIZE.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(&[0; 20]);
        assert!(load_bytes(bytes).is_err());
    }

    #[test]
    fn rejects_entry_hash_that_does_not_match_name() {
        let name = b"manifest.json";
        let entry_offset = HEADER_SIZE + 1;
        let mut bytes = b"YPK\x01".to_vec();
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&entry_offset.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&(name.len() as u32).to_le_bytes());
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(&HEADER_SIZE.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        assert!(load_bytes(bytes).is_err());
    }
}
