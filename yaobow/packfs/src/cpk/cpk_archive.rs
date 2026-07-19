use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use common::SeekRead;
use common::read_ext::ReadExt;
use encoding::Encoding;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    io::{Cursor, Read, Write},
    rc::Rc,
};
use std::{clone::Clone, path::Path};

use crate::memory_file::MemoryFile;

type IoResult<T> = std::io::Result<T>;
type IoError = std::io::Error;
type IoErrorKind = std::io::ErrorKind;

#[allow(dead_code)]
pub struct CpkArchive {
    reader: Box<dyn SeekRead>,
    header: CpkHeader,
    pub entries: Vec<CpkTable>,
    crc_to_index: HashMap<u32, usize>,
    lzo: minilzo_rs::LZO,
}

impl CpkArchive {
    pub fn load(mut reader: Box<dyn SeekRead>) -> IoResult<CpkArchive> {
        let header = CpkHeader::read(&mut reader)?;

        let buffer = if header.is_pal4() {
            let buffer_len = (std::mem::size_of::<CpkTable>() + 4) * header.file_num as usize;
            let mut encrypted_buffer = vec![0; 0x1000];
            reader.read_exact(&mut encrypted_buffer)?;
            let mut decrypted_buffer = xxtea::decrypt_raw(
                &encrypted_buffer,
                "Vampire.C.J at Softstar Technology (ShangHai) Co., Ltd",
            );

            if buffer_len > 0x1000 {
                let mut extra_buffer = vec![0; buffer_len - 0x1000];
                reader.read_exact(&mut extra_buffer)?;
                decrypted_buffer.append(&mut extra_buffer);
            }

            decrypted_buffer
        } else {
            let buffer_len = std::mem::size_of::<CpkTable>() * header.file_num as usize;
            let mut buffer = vec![0; buffer_len];
            reader.read_exact(&mut buffer)?;

            buffer
        };

        let mut entry_cursor = Cursor::new(&buffer);
        let mut entries = Vec::with_capacity(header.file_num as usize);
        for _ in 0..header.file_num {
            entries.push(CpkTable::read(&mut entry_cursor, header.is_pal4())?);
        }

        let crc_to_index = Self::build_index_map(&entries);
        let lzo = minilzo_rs::LZO::init().unwrap();

        Ok(CpkArchive {
            reader,
            header,
            entries,
            crc_to_index,
            lzo,
        })
    }

    pub fn open(&mut self, file_name: &[u8]) -> IoResult<MemoryFile> {
        let hash = super::crc_checksum(file_name);
        let index = self
            .get_entry_index_by_crc(hash)
            .ok_or(IoError::from(IoErrorKind::NotFound))?;

        self.open_entry(index)
    }

    pub fn open_str(&mut self, file_name: &str) -> IoResult<MemoryFile> {
        let file_name = encoding::all::GBK
            .encode(&file_name.to_lowercase(), encoding::EncoderTrap::Strict)
            .map_err(|_| {
                IoError::new(
                    IoErrorKind::InvalidInput,
                    "CPK path cannot be encoded as GBK",
                )
            })?;
        self.open(&file_name)
    }

    pub fn open_first(&mut self) -> IoResult<MemoryFile> {
        self.open_entry(0)
    }

    fn open_entry(&mut self, index: usize) -> IoResult<MemoryFile> {
        let entry = &self.entries[index];
        if entry.is_dir() {
            return Err(IoError::from(IoErrorKind::IsADirectory));
        }

        self.reader
            .seek(std::io::SeekFrom::Start(entry.start_pos as u64))?;
        let mut content = vec![0; entry.packed_size as usize];
        self.reader.read(&mut content)?;
        if !entry.is_compressed() {
            Ok(MemoryFile::new(Cursor::new(content)))
        } else {
            let decompressed_content = self
                .lzo
                .decompress(&content, entry.origin_size as usize)
                .or_else(|e| Err(IoError::new(IoErrorKind::InvalidData, e)))?;
            Ok(MemoryFile::new(Cursor::new(decompressed_content)))
        }
    }

    pub fn build_directory(&mut self) -> IoResult<CpkEntry> {
        let file_names = Self::read_file_names(&mut self.reader, &self.entries)?;
        Ok(Self::build_directory_internal(&self.entries, &file_names))
    }

    pub fn is_pal4(&self) -> bool {
        self.header.data_start == 0x00100080
    }

    /// Leaf (non-hierarchical) name of every entry, in the same order as
    /// `self.entries`. This is a thin public wrapper around the private
    /// name-table reader so tools like [`crate::cpk::CpkRebuilder`] (and
    /// external patchers built on top of it) don't need to re-implement
    /// the on-disk name lookup.
    pub fn file_names(&mut self) -> IoResult<Vec<String>> {
        Self::read_file_names(&mut self.reader, &self.entries)
    }

    /// Full, backslash-separated relative path of every entry (in the
    /// same order as `self.entries`), reconstructed by walking the
    /// `father_crc` chain up to a root entry. The returned strings use
    /// the original (non-lowercased) casing of each path component, so
    /// they are suitable both for display and for feeding back into
    /// [`Self::open_str`] / CRC computation (after lower-casing).
    ///
    /// Orphaned entries (whose `father_crc` doesn't resolve to a known
    /// entry) are best-effort: the walk simply stops early, matching the
    /// permissive behavior of [`Self::build_directory`].
    pub fn full_paths(&mut self) -> IoResult<Vec<String>> {
        let names = self.file_names()?;
        let mut paths = Vec::with_capacity(self.entries.len());

        for start in 0..self.entries.len() {
            let mut components = vec![];
            let mut visited = HashSet::new();
            let mut current = start;

            loop {
                if !visited.insert(current) || current >= names.len() {
                    break;
                }

                components.push(names[current].clone());
                let father_crc = self.entries[current].father_crc;
                if father_crc == 0 {
                    break;
                }

                match self.crc_to_index.get(&father_crc) {
                    Some(&parent_index) => current = parent_index,
                    None => break,
                }
            }

            components.reverse();
            paths.push(components.join("\\"));
        }

        Ok(paths)
    }

    /// Reads the raw (still possibly LZO-compressed) bytes of an entry,
    /// without decompressing. Used by [`crate::cpk::CpkRebuilder`] to copy
    /// unchanged entries verbatim into a rebuilt package.
    pub fn read_packed(&mut self, index: usize) -> IoResult<Vec<u8>> {
        let entry = &self.entries[index];
        self.reader
            .seek(std::io::SeekFrom::Start(entry.start_pos as u64))?;
        let mut content = vec![0; entry.packed_size as usize];
        self.reader.read_exact(&mut content)?;
        Ok(content)
    }

    fn build_directory_internal(entries: &[CpkTable], file_names: &[String]) -> CpkEntry {
        let root_table = CpkTable {
            crc: 0,
            flag: 0,
            father_crc: 0,
            start_pos: 0,
            packed_size: 0,
            origin_size: 0,
            extra_info_size: 0,
        };
        let mut root = CpkEntry::new(root_table, "".to_string());
        let mut crc_to_cpk_entry = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            if i < file_names.len() {
                let cpk_entry = CpkEntry::new(*entry, file_names[i].clone());
                crc_to_cpk_entry.insert(entry.crc, Rc::new(RefCell::new(cpk_entry)));
            }
        }

        for (_, cpk_entry) in &crc_to_cpk_entry {
            if cpk_entry.borrow().raw_entry.father_crc == 0 {
                root.add_child(cpk_entry);
            } else {
                match crc_to_cpk_entry.get(&cpk_entry.borrow().raw_entry.father_crc) {
                    Some(parent_entry) => {
                        parent_entry.borrow_mut().add_child(cpk_entry);
                    }
                    _ => {
                        // Orphan entries.. ignore them
                    }
                }
            }
        }

        root
    }

    fn build_index_map(entries: &Vec<CpkTable>) -> HashMap<u32, usize> {
        let mut map = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            map.insert(entry.crc, i);
        }

        map
    }

    fn _get_entry_by_crc(&self, crc: u32) -> Option<CpkTable> {
        self.crc_to_index
            .get(&crc)
            .and_then(|index| Some(self.entries[*index]))
    }

    fn get_entry_index_by_crc(&self, crc: u32) -> Option<usize> {
        self.crc_to_index.get(&crc).and_then(|index| Some(*index))
    }

    fn read_file_names(cursor: &mut dyn SeekRead, entries: &[CpkTable]) -> IoResult<Vec<String>> {
        let mut names = Vec::with_capacity(entries.len());
        for entry in entries {
            let offset = entry.start_pos + entry.packed_size;
            let length = entry.extra_info_size;
            cursor.seek(std::io::SeekFrom::Start(offset as u64))?;
            let name = cursor
                .read_gbk_string(length as usize)
                .map_err(|_| IoError::from(IoErrorKind::InvalidData))?;
            names.push(name);
        }

        Ok(names)
    }
}

#[allow(dead_code)]
#[derive(Debug)]
struct CpkHeader {
    label: u32,
    version: u32,
    table_start: u32,
    data_start: u32,
    max_file_num: u32,
    file_num: u32,
    is_formatted: u32,
    size_of_header: u32,
    valid_table_num: u32,
    max_table_num: u32,
    fragment_num: u32,
    package_size: u32,
    reserved: [u32; 20],
}

impl CpkHeader {
    pub fn read(cursor: &mut dyn SeekRead) -> IoResult<CpkHeader> {
        let label = cursor.read_u32::<LittleEndian>()?;
        let version = cursor.read_u32::<LittleEndian>()?;
        let table_start = cursor.read_u32::<LittleEndian>()?;
        let data_start = cursor.read_u32::<LittleEndian>()?;
        let max_file_num = cursor.read_u32::<LittleEndian>()?;
        let file_num = cursor.read_u32::<LittleEndian>()?;
        let is_formatted = cursor.read_u32::<LittleEndian>()?;
        let size_of_header = cursor.read_u32::<LittleEndian>()?;
        let valid_table_num = cursor.read_u32::<LittleEndian>()?;
        let max_table_num = cursor.read_u32::<LittleEndian>()?;
        let fragment_num = cursor.read_u32::<LittleEndian>()?;
        let package_size = cursor.read_u32::<LittleEndian>()?;
        let mut reserved: [u32; 20] = Default::default();
        reserved.copy_from_slice(&cursor.read_dw_vec(20)?);

        if label != 0x1A545352 {
            return Err(IoError::from(IoErrorKind::InvalidData));
        }

        Ok(CpkHeader {
            label,
            version,
            table_start,
            data_start,
            max_file_num,
            file_num,
            is_formatted,
            size_of_header,
            valid_table_num,
            max_table_num,
            fragment_num,
            package_size,
            reserved,
        })
    }

    pub fn is_pal4(&self) -> bool {
        self.data_start == 0x00100080
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CpkTable {
    pub(crate) crc: u32,
    pub(crate) flag: u32,
    pub(crate) father_crc: u32,
    pub(crate) start_pos: u32,
    pub(crate) packed_size: u32,
    pub(crate) origin_size: u32,
    pub(crate) extra_info_size: u32,
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum CpkTableFlag {
    None = 0x0,
    IsFile = 0x1,
    IsDir = 0x2,
    Unknown = 0x4,
    Unknown2 = 0x8,
    IsDeleted = 0x10,
    IsNotCompressed = 0x10000,
}

impl CpkTable {
    pub fn read(cursor: &mut dyn SeekRead, extra_ending: bool) -> IoResult<CpkTable> {
        let crc = cursor.read_u32::<LittleEndian>()?;
        let flag = cursor.read_u32::<LittleEndian>()?;
        let father_crc = cursor.read_u32::<LittleEndian>()?;
        let start_pos = cursor.read_u32::<LittleEndian>()?;
        let packed_size = cursor.read_u32::<LittleEndian>()?;
        let origin_size = cursor.read_u32::<LittleEndian>()?;
        let extra_info_size = cursor.read_u32::<LittleEndian>()?;
        if extra_ending {
            cursor.read_u32::<LittleEndian>()?;
        }

        Ok(CpkTable {
            crc,
            flag,
            father_crc,
            start_pos,
            packed_size,
            origin_size,
            extra_info_size,
        })
    }

    pub fn is_compressed(&self) -> bool {
        (self.flag & CpkTableFlag::IsNotCompressed as u32) == 0
    }

    pub fn is_file(&self) -> bool {
        (self.flag & CpkTableFlag::IsFile as u32) != 0
    }

    pub fn is_dir(&self) -> bool {
        (self.flag & CpkTableFlag::IsDir as u32) != 0
    }

    /// Builds a table entry for a directory. Used by
    /// [`crate::cpk::CpkRebuilder`] when synthesizing missing parent
    /// directories for newly added files.
    pub(crate) fn new_dir(crc: u32, father_crc: u32, start_pos: u32, extra_info_size: u32) -> Self {
        CpkTable {
            crc,
            flag: CpkTableFlag::IsDir as u32,
            father_crc,
            start_pos,
            packed_size: 0,
            origin_size: 0,
            extra_info_size,
        }
    }

    /// Builds a table entry for a (possibly compressed) file. Used by
    /// [`crate::cpk::CpkRebuilder`].
    pub(crate) fn new_file(
        crc: u32,
        father_crc: u32,
        start_pos: u32,
        packed_size: u32,
        origin_size: u32,
        extra_info_size: u32,
        compressed: bool,
    ) -> Self {
        let mut flag = CpkTableFlag::IsFile as u32;
        if !compressed {
            flag |= CpkTableFlag::IsNotCompressed as u32;
        }

        CpkTable {
            crc,
            flag,
            father_crc,
            start_pos,
            packed_size,
            origin_size,
            extra_info_size,
        }
    }

    /// Serializes this entry using the non-PAL4 (28-byte) table layout.
    /// PAL4 packages use an extra trailing `u32` and encrypted table, which
    /// [`crate::cpk::CpkRebuilder`] explicitly refuses to write.
    pub(crate) fn write(&self, writer: &mut dyn Write) -> IoResult<()> {
        writer.write_u32::<LittleEndian>(self.crc)?;
        writer.write_u32::<LittleEndian>(self.flag)?;
        writer.write_u32::<LittleEndian>(self.father_crc)?;
        writer.write_u32::<LittleEndian>(self.start_pos)?;
        writer.write_u32::<LittleEndian>(self.packed_size)?;
        writer.write_u32::<LittleEndian>(self.origin_size)?;
        writer.write_u32::<LittleEndian>(self.extra_info_size)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct CpkEntry {
    raw_entry: CpkTable,
    name: String,
    children: Vec<Rc<RefCell<CpkEntry>>>,
}

impl CpkEntry {
    fn new(raw_entry: CpkTable, name: String) -> CpkEntry {
        CpkEntry {
            raw_entry,
            name,
            children: vec![],
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn add_child(&mut self, child: &Rc<RefCell<CpkEntry>>) {
        self.children.push(child.clone());
    }

    pub fn children(&self) -> &[Rc<RefCell<CpkEntry>>] {
        self.children.as_slice()
    }

    pub fn is_dir(&self) -> bool {
        (self.raw_entry.flag & CpkTableFlag::IsDir as u32) != 0
    }

    pub fn ls<P: AsRef<Path>>(&self, path: P) -> std::io::Result<Vec<Rc<RefCell<CpkEntry>>>> {
        let mut components = path.as_ref().components();
        let first = components.next();
        match first {
            Some(component) => {
                let rest = components.as_path();
                let child = self
                    .children
                    .iter()
                    .find(|e| e.borrow().name == component.as_os_str().to_str().unwrap());
                match child {
                    Some(c) => c.borrow().ls(rest),
                    None => Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
                }
            }
            None => Ok(self.children.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn unreadable_file_name_fails_without_shifting_entry_alignment() {
        let mut cursor = Cursor::new(b"second\0".to_vec());
        let entries = [
            CpkTable {
                crc: 0,
                flag: CpkTableFlag::IsFile as u32,
                father_crc: 0,
                start_pos: 1024,
                packed_size: 0,
                origin_size: 0,
                extra_info_size: 8,
            },
            CpkTable {
                crc: 0,
                flag: CpkTableFlag::IsFile as u32,
                father_crc: 0,
                start_pos: 0,
                packed_size: 0,
                origin_size: 0,
                extra_info_size: 7,
            },
        ];

        assert!(CpkArchive::read_file_names(&mut cursor, &entries).is_err());
    }

    #[test]
    fn non_gbk_path_returns_error_instead_of_panicking() {
        let mut archive = CpkArchive {
            reader: Box::new(Cursor::new(Vec::new())),
            header: CpkHeader {
                label: 0,
                version: 0,
                table_start: 0,
                data_start: 0,
                max_file_num: 0,
                file_num: 0,
                is_formatted: 0,
                size_of_header: 0,
                valid_table_num: 0,
                max_table_num: 0,
                fragment_num: 0,
                package_size: 0,
                reserved: [0; 20],
            },
            entries: Vec::new(),
            crc_to_index: HashMap::new(),
            lzo: minilzo_rs::LZO::init().unwrap(),
        };

        let error = archive.open_str("emoji-\u{1f600}.mv3").err().unwrap();
        assert_eq!(error.kind(), IoErrorKind::InvalidInput);
    }

    #[test]
    fn truncated_header_returns_error_instead_of_panicking() {
        let error = CpkArchive::load(Box::new(Cursor::new(vec![0; 16])))
            .err()
            .expect("truncated CPK must fail");
        assert_eq!(error.kind(), IoErrorKind::UnexpectedEof);
    }
}
