//! PAL3-only `.cpk` rebuilder.
//!
//! PAL3's `.cpk` packages are a flat, on-disk hash table:
//!
//! ```text
//! [ CpkHeader (128 bytes) ]
//! [ CpkTable entry 0 ][ CpkTable entry 1 ] ... [ CpkTable entry N-1 ]   (28 bytes each, PAL3)
//! [ entry 0 data ][ entry 0 name (GBK) ]
//! [ entry 1 data ][ entry 1 name (GBK) ]
//! ...
//! ```
//!
//! Every entry (file *and* directory) is identified purely by
//! `crc = crc_checksum(gbk_encode(lowercase(full_backslash_path)))`, and
//! linked to its parent directory via `father_crc` (0 for top-level
//! entries). There is no requirement that entries appear in the table in
//! any particular order, or that a directory's children be contiguous —
//! [`CpkArchive::build_directory`] reconstructs the hierarchy purely from
//! `crc`/`father_crc` pairs. This is what makes an additive rebuild
//! practical: unchanged entries can be copied byte-for-byte (content +
//! name) into the new package, only their `start_pos` needs to shift to
//! account for the new table size / newly added entries.
//!
//! PAL4 packages reuse the same `CpkTable` layout plus a trailing `u32`
//! per entry and an XXTEA-encrypted table; [`CpkRebuilder`] refuses to
//! touch those (see [`CpkArchive::is_pal4`]).
//!
//! This module intentionally exposes only the [`CpkEdit`] /
//! [`CpkRebuilder`] surface — a higher level "patcher" (e.g. something
//! driving edits from [`crate::AssetCatalog::resolve`]) should not need
//! to know about `CpkTable` internals at all.

use std::{
    collections::HashMap,
    io::{Read, Write},
    path::Path,
};

use byteorder::{LittleEndian, WriteBytesExt};
use encoding::{EncoderTrap, Encoding};

use crate::create_reader;

use super::{
    cpk_archive::{CpkArchive, CpkTable},
    crc_checksum,
};

type IoResult<T> = std::io::Result<T>;

/// A single change to apply while rebuilding a package.
#[derive(Debug, Clone)]
pub enum CpkEdit {
    /// Create or replace the file at `path` (backslash or forward-slash
    /// separated, relative to the package root) with `data`. Missing
    /// parent directories are created automatically.
    File { path: String, data: Vec<u8> },
    /// Ensure an (possibly new, possibly already existing) directory
    /// exists at `path`. Missing parent directories are created
    /// automatically. This is a no-op if the directory is already
    /// present in the source package.
    Directory { path: String },
    /// Remove an existing file at `path`.
    RemoveFile { path: String },
    /// Remove an existing empty directory at `path`.
    RemoveDirectory { path: String },
}

impl CpkEdit {
    pub fn file(path: impl Into<String>, data: Vec<u8>) -> Self {
        CpkEdit::File {
            path: path.into(),
            data,
        }
    }

    pub fn directory(path: impl Into<String>) -> Self {
        CpkEdit::Directory { path: path.into() }
    }

    pub fn remove_file(path: impl Into<String>) -> Self {
        CpkEdit::RemoveFile { path: path.into() }
    }

    pub fn remove_directory(path: impl Into<String>) -> Self {
        CpkEdit::RemoveDirectory { path: path.into() }
    }

    fn path(&self) -> &str {
        match self {
            CpkEdit::File { path, .. } => path,
            CpkEdit::Directory { path } => path,
            CpkEdit::RemoveFile { path } => path,
            CpkEdit::RemoveDirectory { path } => path,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum CpkRebuildError {
    #[error("rebuilding PAL4 .cpk packages is not supported")]
    Pal4NotSupported,
    #[error("path {0:?} is empty or only contains path separators")]
    EmptyPath(String),
    #[error("path {0:?} cannot be represented in GBK")]
    InvalidPath(String),
    #[error("path {0:?} already exists as a {1} in the source package")]
    KindConflict(String, &'static str),
    #[error("path {0:?} does not exist in the source package")]
    MissingPath(String),
    #[error("directory {0:?} is not empty")]
    DirectoryNotEmpty(String),
    #[error(
        "crc32 collision: {0:?} and {1:?} hash to the same crc (0x{2:08x}); \
         cannot add both to the same package"
    )]
    CrcCollision(String, String, u32),
    #[error("verification failed: content of {0:?} does not match the requested edit")]
    VerificationFailed(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Lzo(#[from] minilzo_rs::Error),
}

type Result<T> = std::result::Result<T, CpkRebuildError>;

/// A planned output entry: either copied verbatim from the source
/// package, or (re)created from an edit.
struct PlannedEntry {
    crc: u32,
    father_crc: u32,
    is_dir: bool,
    /// Already-encoded (GBK) leaf name.
    name_bytes: Vec<u8>,
    /// Final on-disk bytes (possibly LZO-compressed). Empty for
    /// directories.
    content: Vec<u8>,
    origin_size: u32,
    compressed: bool,
    /// Backslash-joined, original-case full path. Kept for error
    /// messages / conflict detection only.
    full_path: String,
    removed: bool,
}

/// Rebuilds PAL3 `.cpk` archives.
///
/// This is the primitive a higher-level patcher should drive: resolve a
/// VFS path to a physical package with [`crate::AssetCatalog::resolve`],
/// gather the desired [`CpkEdit`]s, and call [`CpkRebuilder::rebuild`] to
/// produce a new package on disk.
pub struct CpkRebuilder;

impl CpkRebuilder {
    /// Rebuilds `source_path` into `dest_path`, applying `edits`.
    ///
    /// Unchanged entries are copied verbatim (no decompress/recompress
    /// round-trip). Replaced/added files are LZO-compressed when that
    /// shrinks the data, otherwise stored uncompressed (a deterministic
    /// choice based on compressed vs. original size, not a random or
    /// best-effort fallback). After writing, every edited file is
    /// reopened from the freshly-written package and compared
    /// byte-for-byte against the requested content.
    pub fn rebuild<P: AsRef<Path>, Q: AsRef<Path>>(
        source_path: P,
        dest_path: Q,
        edits: &[CpkEdit],
    ) -> Result<()> {
        let reader = create_reader(source_path.as_ref())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let mut archive = CpkArchive::load(reader)?;

        if archive.is_pal4() {
            return Err(CpkRebuildError::Pal4NotSupported);
        }

        let planned = Self::plan(&mut archive, edits)?;
        Self::write_package(dest_path.as_ref(), &planned)?;
        Self::verify(dest_path.as_ref(), edits)?;

        Ok(())
    }

    fn plan(archive: &mut CpkArchive, edits: &[CpkEdit]) -> Result<Vec<PlannedEntry>> {
        let names = archive.file_names()?;
        let full_paths = archive.full_paths()?;

        let mut planned = Vec::with_capacity(archive.entries.len() + edits.len());
        let mut index_of_lower_path: HashMap<String, usize> = HashMap::new();
        let mut index_of_crc: HashMap<u32, usize> = HashMap::new();

        // Snapshot the (crc, father_crc, is_dir, origin_size, compressed)
        // tuple for every source entry up front: `read_packed` needs
        // `&mut archive`, which would otherwise conflict with holding a
        // borrow of `archive.entries` across the loop.
        let entry_meta: Vec<(u32, u32, bool, u32, bool)> = archive
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.crc,
                    entry.father_crc,
                    entry.is_dir(),
                    entry.origin_size,
                    entry.is_compressed(),
                )
            })
            .collect();

        for (i, (crc, father_crc, is_dir, origin_size, compressed)) in
            entry_meta.into_iter().enumerate()
        {
            let full_path = full_paths.get(i).cloned().unwrap_or_default();
            let name = names.get(i).cloned().unwrap_or_default();
            let content = if is_dir {
                vec![]
            } else {
                archive.read_packed(i)?
            };

            let name_bytes = encode_gbk(&name)?;
            index_of_lower_path.insert(full_path.to_lowercase(), planned.len());
            index_of_crc.entry(crc).or_insert(planned.len());
            planned.push(PlannedEntry {
                crc,
                father_crc,
                is_dir,
                name_bytes,
                content,
                origin_size,
                compressed,
                full_path,
                removed: false,
            });
        }

        let mut lzo = minilzo_rs::LZO::init().map_err(CpkRebuildError::Lzo)?;

        for edit in edits {
            Self::apply_edit(
                &mut planned,
                &mut index_of_lower_path,
                &mut index_of_crc,
                &mut lzo,
                edit,
            )?;
        }

        Ok(planned.into_iter().filter(|entry| !entry.removed).collect())
    }

    /// Pushes a brand-new (not-a-replace) entry, guarding against a crc32
    /// collision with an unrelated, differently-cased-but-distinct path
    /// already planned (extremely unlikely with `crc_checksum`'s 32-bit
    /// space, but silently overwriting the wrong entry would corrupt the
    /// rebuilt package, so this is checked rather than assumed away).
    fn push_new_entry(
        planned: &mut Vec<PlannedEntry>,
        index_of_lower_path: &mut HashMap<String, usize>,
        index_of_crc: &mut HashMap<u32, usize>,
        entry: PlannedEntry,
    ) -> Result<()> {
        if let Some(&existing) = index_of_crc.get(&entry.crc) {
            if planned[existing].full_path.to_lowercase() != entry.full_path.to_lowercase() {
                return Err(CpkRebuildError::CrcCollision(
                    planned[existing].full_path.clone(),
                    entry.full_path,
                    entry.crc,
                ));
            }
        }

        index_of_lower_path.insert(entry.full_path.to_lowercase(), planned.len());
        index_of_crc.insert(entry.crc, planned.len());
        planned.push(entry);
        Ok(())
    }

    fn apply_edit(
        planned: &mut Vec<PlannedEntry>,
        index_of_lower_path: &mut HashMap<String, usize>,
        index_of_crc: &mut HashMap<u32, usize>,
        lzo: &mut minilzo_rs::LZO,
        edit: &CpkEdit,
    ) -> Result<()> {
        let raw_path = edit.path();
        let normalized = raw_path.replace('/', "\\");
        let components: Vec<&str> = normalized.split('\\').filter(|c| !c.is_empty()).collect();

        if components.is_empty() {
            return Err(CpkRebuildError::EmptyPath(raw_path.to_string()));
        }

        let removes_entry = matches!(
            edit,
            CpkEdit::RemoveFile { .. } | CpkEdit::RemoveDirectory { .. }
        );

        // Ensure every ancestor directory exists for additive edits,
        // tracking the crc of the immediate parent as we descend.
        let mut parent_crc = 0u32;
        for depth in 0..components.len() - 1 {
            let acc_path = components[..=depth].join("\\");
            let lower = acc_path.to_lowercase();

            parent_crc = match index_of_lower_path.get(&lower) {
                Some(&idx) => {
                    if planned[idx].removed {
                        return Err(CpkRebuildError::MissingPath(acc_path));
                    }
                    if !planned[idx].is_dir {
                        return Err(CpkRebuildError::KindConflict(acc_path, "file"));
                    }
                    planned[idx].crc
                }
                None if removes_entry => return Err(CpkRebuildError::MissingPath(acc_path)),
                None => {
                    let crc = crc_for_path(&acc_path)?;
                    let name_bytes = encode_gbk(components[depth])?;
                    Self::push_new_entry(
                        planned,
                        index_of_lower_path,
                        index_of_crc,
                        PlannedEntry {
                            crc,
                            father_crc: parent_crc,
                            is_dir: true,
                            name_bytes,
                            content: vec![],
                            origin_size: 0,
                            compressed: false,
                            full_path: acc_path,
                            removed: false,
                        },
                    )?;
                    crc
                }
            };
        }

        let leaf_full_path = components.join("\\");
        let leaf_lower = leaf_full_path.to_lowercase();
        let leaf_name = *components.last().unwrap();

        match edit {
            CpkEdit::Directory { .. } => {
                match index_of_lower_path.get(&leaf_lower) {
                    Some(&idx) => {
                        if planned[idx].removed {
                            return Err(CpkRebuildError::MissingPath(leaf_full_path));
                        }
                        if !planned[idx].is_dir {
                            return Err(CpkRebuildError::KindConflict(leaf_full_path, "file"));
                        }
                        // Directory already present: nothing to do.
                    }
                    None => {
                        let crc = crc_for_path(&leaf_full_path)?;
                        let name_bytes = encode_gbk(leaf_name)?;
                        Self::push_new_entry(
                            planned,
                            index_of_lower_path,
                            index_of_crc,
                            PlannedEntry {
                                crc,
                                father_crc: parent_crc,
                                is_dir: true,
                                name_bytes,
                                content: vec![],
                                origin_size: 0,
                                compressed: false,
                                full_path: leaf_full_path,
                                removed: false,
                            },
                        )?;
                    }
                }
            }
            CpkEdit::File { data, .. } => {
                let (content, compressed) = compress_or_store(lzo, data)?;

                match index_of_lower_path.get(&leaf_lower) {
                    Some(&idx) => {
                        if planned[idx].removed {
                            return Err(CpkRebuildError::MissingPath(leaf_full_path));
                        }
                        if planned[idx].is_dir {
                            return Err(CpkRebuildError::KindConflict(leaf_full_path, "directory"));
                        }
                        // Replace in place: crc/father_crc/name are unchanged.
                        planned[idx].content = content;
                        planned[idx].compressed = compressed;
                        planned[idx].origin_size = data.len() as u32;
                    }
                    None => {
                        let crc = crc_for_path(&leaf_full_path)?;
                        let name_bytes = encode_gbk(leaf_name)?;
                        Self::push_new_entry(
                            planned,
                            index_of_lower_path,
                            index_of_crc,
                            PlannedEntry {
                                crc,
                                father_crc: parent_crc,
                                is_dir: false,
                                name_bytes,
                                content,
                                origin_size: data.len() as u32,
                                compressed,
                                full_path: leaf_full_path,
                                removed: false,
                            },
                        )?;
                    }
                }
            }
            CpkEdit::RemoveFile { .. } => {
                let Some(idx) = index_of_lower_path.remove(&leaf_lower) else {
                    return Err(CpkRebuildError::MissingPath(leaf_full_path));
                };
                if planned[idx].removed {
                    return Err(CpkRebuildError::MissingPath(leaf_full_path));
                }
                if planned[idx].is_dir {
                    return Err(CpkRebuildError::KindConflict(leaf_full_path, "directory"));
                }
                index_of_crc.remove(&planned[idx].crc);
                planned[idx].removed = true;
            }
            CpkEdit::RemoveDirectory { .. } => {
                let Some(idx) = index_of_lower_path.get(&leaf_lower).copied() else {
                    return Err(CpkRebuildError::MissingPath(leaf_full_path));
                };
                if planned[idx].removed {
                    return Err(CpkRebuildError::MissingPath(leaf_full_path));
                }
                if !planned[idx].is_dir {
                    return Err(CpkRebuildError::KindConflict(leaf_full_path, "file"));
                }
                let crc = planned[idx].crc;
                if planned
                    .iter()
                    .any(|entry| !entry.removed && entry.father_crc == crc)
                {
                    return Err(CpkRebuildError::DirectoryNotEmpty(leaf_full_path));
                }
                index_of_lower_path.remove(&leaf_lower);
                index_of_crc.remove(&crc);
                planned[idx].removed = true;
            }
        }

        Ok(())
    }

    fn write_package(dest_path: &Path, planned: &[PlannedEntry]) -> IoResult<()> {
        const HEADER_SIZE: u32 = 128;
        const TABLE_ENTRY_SIZE: u32 = 28;

        let file_num = planned.len() as u32;
        let table_start = HEADER_SIZE;
        let data_start = table_start + file_num * TABLE_ENTRY_SIZE;

        // Lay out the data segment sequentially: content immediately
        // followed by the entry's (GBK-encoded) name, mirroring how
        // `CpkArchive::read_file_names` locates names via
        // `start_pos + packed_size`.
        let mut tables = Vec::with_capacity(planned.len());
        let mut offset = data_start;
        for entry in planned {
            let packed_size = entry.content.len() as u32;
            let extra_info_size = entry.name_bytes.len() as u32;

            let table = if entry.is_dir {
                CpkTable::new_dir(entry.crc, entry.father_crc, offset, extra_info_size)
            } else {
                CpkTable::new_file(
                    entry.crc,
                    entry.father_crc,
                    offset,
                    packed_size,
                    entry.origin_size,
                    extra_info_size,
                    entry.compressed,
                )
            };

            offset += packed_size + extra_info_size;
            tables.push(table);
        }
        let package_size = offset;

        let mut writer = std::io::BufWriter::new(std::fs::File::create(dest_path)?);

        write_header(&mut writer, table_start, data_start, file_num, package_size)?;

        for table in &tables {
            table.write(&mut writer)?;
        }

        for entry in planned {
            writer.write_all(&entry.content)?;
            writer.write_all(&entry.name_bytes)?;
        }

        writer.flush()?;
        Ok(())
    }

    fn verify(dest_path: &Path, edits: &[CpkEdit]) -> Result<()> {
        let reader = create_reader(dest_path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let mut archive = CpkArchive::load(reader)?;

        for edit in edits {
            let normalized = edit.path().replace('/', "\\");
            match edit {
                CpkEdit::File { data, .. } => {
                    let mut file = archive
                        .open_str(&normalized)
                        .map_err(|_| CpkRebuildError::VerificationFailed(normalized.clone()))?;

                    let mut buf = Vec::with_capacity(data.len());
                    file.read_to_end(&mut buf)?;

                    if &buf != data {
                        return Err(CpkRebuildError::VerificationFailed(normalized));
                    }
                }
                CpkEdit::RemoveFile { .. } => {
                    if archive.open_str(&normalized).is_ok() {
                        return Err(CpkRebuildError::VerificationFailed(normalized));
                    }
                }
                CpkEdit::RemoveDirectory { .. } => {
                    let lower = normalized.to_lowercase();
                    if archive
                        .full_paths()?
                        .iter()
                        .any(|path| path.to_lowercase() == lower)
                    {
                        return Err(CpkRebuildError::VerificationFailed(normalized));
                    }
                }
                CpkEdit::Directory { .. } => {}
            }
        }

        Ok(())
    }
}

fn write_header(
    writer: &mut impl Write,
    table_start: u32,
    data_start: u32,
    file_num: u32,
    package_size: u32,
) -> IoResult<()> {
    writer.write_u32::<LittleEndian>(0x1A545352)?; // label ("RST\x1A")
    writer.write_u32::<LittleEndian>(1)?; // version
    writer.write_u32::<LittleEndian>(table_start)?;
    writer.write_u32::<LittleEndian>(data_start)?;
    writer.write_u32::<LittleEndian>(file_num)?; // max_file_num
    writer.write_u32::<LittleEndian>(file_num)?;
    writer.write_u32::<LittleEndian>(1)?; // is_formatted
    writer.write_u32::<LittleEndian>(table_start)?; // size_of_header
    writer.write_u32::<LittleEndian>(file_num)?; // valid_table_num
    writer.write_u32::<LittleEndian>(file_num)?; // max_table_num
    writer.write_u32::<LittleEndian>(0)?; // fragment_num
    writer.write_u32::<LittleEndian>(package_size)?;
    for _ in 0..20 {
        writer.write_u32::<LittleEndian>(0)?; // reserved
    }

    Ok(())
}

/// Compresses `data` with LZO1X; falls back to storing it uncompressed
/// whenever compression doesn't strictly shrink the payload (or fails
/// outright, e.g. `Error::NotCompressible`). This is a deterministic
/// choice driven purely by output size, not a random/best-effort guess.
fn compress_or_store(lzo: &mut minilzo_rs::LZO, data: &[u8]) -> Result<(Vec<u8>, bool)> {
    if data.is_empty() {
        return Ok((vec![], false));
    }

    match lzo.compress(data) {
        Ok(compressed) if compressed.len() < data.len() => Ok((compressed, true)),
        Ok(_) => Ok((data.to_vec(), false)),
        Err(minilzo_rs::Error::NotCompressible) => Ok((data.to_vec(), false)),
        Err(e) => Err(CpkRebuildError::Lzo(e)),
    }
}

/// `crc_checksum(gbk_encode(lowercase(path)))`, matching the lookup used
/// by [`CpkArchive::open_str`] / [`super::cpk_fs::CpkFs::open_path`].
fn crc_for_path(path: &str) -> Result<u32> {
    let bytes = encode_gbk(&path.to_lowercase())?;
    Ok(crc_checksum(&bytes))
}

fn encode_gbk(s: &str) -> Result<Vec<u8>> {
    encoding::all::GBK
        .encode(s, EncoderTrap::Strict)
        .map_err(|_| CpkRebuildError::InvalidPath(s.to_string()))
}
