use std::{
    io::{Cursor, Read},
    path::Path,
};

use byteorder::ReadBytesExt;
use common::read_ext::ReadExt;
use encoding::Encoding;
use radiance::utils::SeekRead;

use crate::fs::memory_file::MemoryFile;

pub struct PkgArchive {
    reader: Box<dyn SeekRead>,
    _header: PkgHeader,
    pub entries: PkgEntries,
}

impl PkgArchive {
    pub fn load(mut reader: Box<dyn SeekRead>, decrypt_key: &str) -> anyhow::Result<PkgArchive> {
        let _header = PkgHeader::read(&mut reader)?;
        reader.seek(std::io::SeekFrom::Start(_header.entries_start as u64))?;

        let entries = PkgEntries::read(&mut reader, &decrypt_key)?;
        Ok(Self {
            reader,
            _header,
            entries,
        })
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<MemoryFile> {
        let path = path.as_ref().as_os_str().to_string_lossy().to_lowercase();
        let mut path = path.replace('/', "\\");
        if !path.starts_with("\\") {
            path = format!("\\{}", path);
        }

        for entry in &self.entries.file_entries {
            if entry.fullpath.to_lowercase() == path {
                self.reader
                    .seek(std::io::SeekFrom::Start(entry.start_position as u64))?;
                let mut data = self.reader.read_u8_vec(entry.size as usize)?;
                if entry.size != entry.size2 {
                    data = miniz_oxide::inflate::decompress_to_vec_zlib(&data).map_err(|_| {
                        PkgReadError::DecompressionError("Unable to decompress file".to_string())
                    })?;
                }
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
pub struct PkgFileEntry {
    pub fullpath: String,
    pub filename: String,
    pub start_position: u32,
    pub size: u32,
    pub size2: u32,
}

impl PkgFileEntry {
    pub fn new(fullpath: String, start_position: u32, size: u32, size2: u32) -> Self {
        Self {
            fullpath: fullpath.clone(),
            filename: fullpath.split("\\").last().unwrap_or("").to_string(),
            start_position,
            size,
            size2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PkgFolderEntry {
    pub name: String,
    pub children: Vec<PkgEntry>,
}

impl PkgFolderEntry {
    pub fn list_content<P: AsRef<Path>>(&self, path: P) -> std::io::Result<Vec<PkgEntry>> {
        let mut components = path.as_ref().components();
        let first = components.next();
        match first {
            Some(component) => {
                let rest = components.as_path();
                for entry in &self.children {
                    match entry {
                        PkgEntry::File(_) => {
                            return Err(std::io::Error::from(std::io::ErrorKind::NotADirectory))
                        }
                        PkgEntry::Folder(f) => {
                            if f.name.as_str() == component.as_os_str().to_str().unwrap() {
                                return f.list_content(rest);
                            }
                        }
                    }
                }

                return Err(std::io::Error::from(std::io::ErrorKind::NotFound));
            }
            None => Ok(self.children.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum PkgEntry {
    File(PkgFileEntry),
    Folder(PkgFolderEntry),
}

impl PkgEntry {
    pub fn name(&self) -> &str {
        match self {
            PkgEntry::File(f) => &f.filename,
            PkgEntry::Folder(f) => &f.name,
        }
    }

    pub fn as_folder_mut(&mut self) -> Option<&mut PkgFolderEntry> {
        match self {
            PkgEntry::File(_) => None,
            PkgEntry::Folder(f) => Some(f),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PkgEntries {
    pub total_length: u32,
    pub md5hash: Vec<u8>,
    pub file_entries: Vec<PkgFileEntry>,
    pub root_entry: PkgFolderEntry,
}

impl PkgEntries {
    pub fn read(reader: &mut dyn Read, decrypt_key: &str) -> anyhow::Result<Self> {
        let decrypt_key = Self::preprocess_decrypt_key(decrypt_key);

        let total_length = reader.read_u32_le()?;
        let mut md5hash = reader.read_u8_vec(0x20)?;

        for i in 0..0x20 {
            md5hash[i] ^= 0xa4;
        }

        let _ = encoding::all::GBK.decode(
            &md5hash
                .clone()
                .into_iter()
                .take_while(|&c| c != 0)
                .collect::<Vec<u8>>(),
            encoding::DecoderTrap::Ignore,
        );

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
        if md5hash == &*md5_computed {
            Err(PkgReadError::HashMismatch)?;
        }

        let buf = miniz_oxide::inflate::decompress_to_vec_zlib(&buf)
            .map_err(|err| PkgReadError::DecompressionError(format!("{:?}", err)))?;

        let mut file_entries = vec![];
        let mut entry_cursor = Cursor::new(buf);
        while !entry_cursor.is_empty() {
            let name_len = entry_cursor.read_u32_le()?;
            let name = String::from_utf8(entry_cursor.read_u8_vec(name_len as usize)?)?;
            let start_position = entry_cursor.read_u32_le()?;
            let size = entry_cursor.read_u32_le()?;
            let size2 = entry_cursor.read_u32_le()?;
            file_entries.push(PkgFileEntry::new(name, start_position, size, size2));
        }

        let root_entry = Self::construct_folder_structure(&file_entries);

        Ok(Self {
            total_length,
            md5hash,
            file_entries,
            root_entry,
        })
    }

    fn preprocess_decrypt_key(key: &str) -> Vec<u8> {
        let mut key = Vec::from(key.as_bytes());
        let pivot = key[key.len() / 2] ^ 0xa4;

        for i in 0..key.len() {
            key[i] ^= pivot;
        }

        let last = key.len() - 1;
        key.swap(0, last);
        key
    }

    fn construct_folder_structure(file_entries: &Vec<PkgFileEntry>) -> PkgFolderEntry {
        let mut root_children = vec![];
        for entry in file_entries {
            let path = entry.fullpath.trim_start_matches('\\');
            let components = path.split('\\').collect();
            Self::create_entry_for(entry, &mut root_children, components);
        }

        PkgFolderEntry {
            name: "/".to_string(),
            children: root_children,
        }
    }

    fn create_entry_for(
        file_entry: &PkgFileEntry,
        current: &mut Vec<PkgEntry>,
        mut path_components: Vec<&str>,
    ) {
        let c = path_components.remove(0);

        if path_components.len() == 0 {
            current.push(PkgEntry::File(file_entry.clone()));
        } else {
            let mut children = None;
            for child in current.iter_mut() {
                match child {
                    PkgEntry::File(_) => {}
                    PkgEntry::Folder(f) => {
                        if f.name == *c {
                            children = Some(&mut f.children);
                            break;
                        }
                    }
                }
            }

            if children.is_none() {
                current.push(PkgEntry::Folder(PkgFolderEntry {
                    name: (*c).to_owned(),
                    children: vec![],
                }));
                children = current
                    .last_mut()
                    .unwrap()
                    .as_folder_mut()
                    .and_then(|f| Some(&mut f.children));
            }

            Self::create_entry_for(file_entry, children.unwrap(), path_components);
        }
    }
}
