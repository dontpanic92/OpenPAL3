use std::{
    io::{Cursor, Read},
    path::Path,
};

use byteorder::ReadBytesExt;
use common::read_ext::ReadExt;
use encoding::Encoding;

use crate::fs::memory_file::MemoryFile;

#[derive(Debug)]
pub struct PkgArchive<T: AsRef<[u8]>> {
    cursor: Cursor<T>,
    header: PkgHeader,
    pub entries: PkgEntires,
}

impl<T: AsRef<[u8]>> PkgArchive<T> {
    pub fn load(mut cursor: Cursor<T>, decrypt_key: &str) -> anyhow::Result<PkgArchive<T>> {
        let header = PkgHeader::read(&mut cursor)?;
        cursor.set_position(header.entries_start as u64);

        let entries = PkgEntires::read(&mut cursor, &decrypt_key)?;
        Ok(Self {
            cursor,
            header,
            entries,
        })
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<MemoryFile> {
        let path = path.as_ref().as_os_str().to_string_lossy();
        let mut path = path.replace('/', "\\");
        if !path.starts_with("\\") {
            path = format!("\\{}", path);
        }

        for entry in &self.entries.entries {
            if entry.filename == path {
                self.cursor.set_position(entry.start_position as u64);
                let data = self.cursor.read_u8_vec(entry.size as usize)?;
                return Ok(MemoryFile::new(Cursor::new(data)));
            }
        }

        Err(std::io::Error::from(std::io::ErrorKind::NotFound))?
    }
}

#[derive(thiserror::Error, Debug)]
pub enum PkgReadError {
    #[error("Incorrect magic found for pkg. Should be: 0x4b50, found {0}")]
    IncorrectMagic(u16),
    #[error("Only 3.0 version is supported, found {0}")]
    UnsupportedVersion(u32),
    #[error("Hash mismatch")]
    HashMismatch,
    #[error("Error decompression {0}")]
    DecompressionError(String),
}

#[derive(Debug, Clone)]
pub struct PkgHeader {
    pub magic: u16,
    pub version: u32,
    pub unknown: u16,
    pub unknown2: u32,
    pub unknown3: u32,
    pub entries_start: u32,
    pub unknown_byte: u8,
    pub unknown_byte2: u8,
    pub unknown_byte3: u8,
    pub unknown_byte4: u8,
}

impl PkgHeader {
    pub fn read(reader: &mut dyn Read) -> anyhow::Result<Self> {
        let magic = reader.read_u16_le()?;
        if magic != 0x4b50 {
            Err(PkgReadError::IncorrectMagic(magic))?;
        }

        let version = reader.read_u32_le()?;
        if version != 0x00302e33 {
            Err(PkgReadError::UnsupportedVersion(version))?;
        }

        let unknown = reader.read_u16_le()?;
        let unknown2 = reader.read_u32_le()?;
        let unknown3 = reader.read_u32_le()?;

        let entries_start = reader.read_u32_le()?;
        let unknown_byte = reader.read_u8()?;
        let unknown_byte2 = reader.read_u8()?;
        let unknown_byte3 = reader.read_u8()?;
        let unknown_byte4 = reader.read_u8()?;

        Ok(Self {
            magic,
            version,
            unknown,
            unknown2,
            unknown3,
            entries_start,
            unknown_byte,
            unknown_byte2,
            unknown_byte3,
            unknown_byte4,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PkgEntry {
    pub filename: String,
    pub start_position: u32,
    pub size: u32,
    pub size2: u32,
}

impl PkgEntry {
    pub fn new(filename: String, start_position: u32, size: u32, size2: u32) -> Self {
        Self {
            filename,
            start_position,
            size,
            size2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PkgEntires {
    pub total_length: u32,
    pub md5hash: Vec<u8>,
    pub entries: Vec<PkgEntry>,
}

impl PkgEntires {
    pub fn read(reader: &mut dyn Read, decrypt_key: &str) -> anyhow::Result<Self> {
        let decrypt_key = Self::preprocess_decrypt_key(decrypt_key);

        let total_length = reader.read_u32_le()?;
        let mut md5hash = reader.read_u8_vec(0x20)?;

        for i in 0..0x20 {
            md5hash[i] ^= 0xa4;
        }

        let s = encoding::all::GBK.decode(
            &md5hash
                .clone()
                .into_iter()
                .take_while(|&c| c != 0)
                .collect::<Vec<u8>>(),
            encoding::DecoderTrap::Ignore,
        );

        println!("{:0x?} {:?}", md5hash, s);

        let mut buf = vec![];
        let _ = reader.read_to_end(&mut buf)?;

        for i in 0..buf.len() {
            buf[i] ^= decrypt_key[i % decrypt_key.len()];
        }

        let mut context = md5::Context::new();
        let chunks = buf.chunks(0x400);
        for chunk in chunks {
            context.consume(chunk);
        }

        let md5_computed = context.compute();
        println!("{:?}", md5_computed);
        if md5hash == &*md5_computed {
            Err(PkgReadError::HashMismatch)?;
        }

        let buf = miniz_oxide::inflate::decompress_to_vec_zlib(&buf)
            .map_err(|err| PkgReadError::DecompressionError(format!("{:?}", err)))?;

        let mut entries = vec![];
        let mut entry_cursor = Cursor::new(buf);
        while !entry_cursor.is_empty() {
            let name_len = entry_cursor.read_u32_le()?;
            let name = String::from_utf8(entry_cursor.read_u8_vec(name_len as usize)?)?;
            let start_position = entry_cursor.read_u32_le()?;
            let size = entry_cursor.read_u32_le()?;
            let size2 = entry_cursor.read_u32_le()?;
            entries.push(PkgEntry::new(name, start_position, size, size2));
        }

        Ok(Self {
            total_length,
            md5hash,
            entries,
        })
    }

    fn preprocess_decrypt_key(key: &str) -> Vec<u8> {
        let mut key = Vec::from(key.as_bytes());
        println!("key: {:?}", &key);
        let pivot = key[key.len() / 2] ^ 0xa4;

        for i in 0..key.len() {
            key[i] ^= pivot;
        }

        let last = key.len() - 1;
        key.swap(0, last);
        key
    }
}
