//! Test-only fixture builders: synthetic (non-PAL4) `.cpk` packages and
//! matching `.yapatch` files, so `tests/` can exercise the full
//! transaction engine without any real game assets.
//!
//! The `.cpk` builder is independent of `packfs::cpk`'s own writer
//! (`CpkRebuilder`) — it re-derives the on-disk layout directly from
//! the documented format using only public API (`crc_checksum`), the
//! same approach `packfs/tests/cpk_rebuild.rs` uses (that file can't be
//! imported across crates, so this is a from-scratch equivalent, not a
//! copy).
//!
//! Gated behind the `test-support` feature (see `Cargo.toml`) so this
//! never ships in the production library or the GUI binary, but is
//! still reachable both from this crate's own `#[cfg(test)]` modules
//! and from `tests/*.rs` integration tests (which depend on this crate
//! with `features = ["test-support"]`, see the `[dev-dependencies]`
//! self-dependency).

use std::path::{Path, PathBuf};

use byteorder::{LittleEndian, WriteBytesExt};
use encoding::{EncoderTrap, Encoding};

use asset_project::hash::ContentHash;
use asset_project::manifest::{AssetChange, AssetChangeKind, PackagePath, TargetPackage};
use asset_project::patch::{PackageFingerprint, PatchManifest, publish};
use packfs::cpk::crc_checksum;

const FLAG_IS_FILE: u32 = 0x1;
const FLAG_IS_DIR: u32 = 0x2;
const FLAG_NOT_COMPRESSED: u32 = 0x10000;
const HEADER_SIZE: u32 = 128;
const TABLE_ENTRY_SIZE: u32 = 28;

fn gbk(s: &str) -> Vec<u8> {
    encoding::all::GBK.encode(s, EncoderTrap::Strict).unwrap()
}

fn gbk_lower(s: &str) -> Vec<u8> {
    gbk(&s.to_lowercase())
}

fn crc_of(path: &str) -> u32 {
    crc_checksum(&gbk_lower(path))
}

fn father_crc_of(path: &str) -> u32 {
    match path.rsplit_once('\\') {
        Some((parent, _)) => crc_of(parent),
        None => 0,
    }
}

fn leaf_name(path: &str) -> &str {
    path.rsplit('\\').next().unwrap()
}

struct RawEntry {
    crc: u32,
    father_crc: u32,
    flag: u32,
    content: Vec<u8>,
    name: Vec<u8>,
    origin_size: u32,
}

/// Hand-builds a minimal, valid non-PAL4 `.cpk` byte stream containing
/// `files` (backslash-separated relative paths -> content). Ancestor
/// directory entries are synthesized automatically.
pub fn build_fixture_cpk(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut dirs: Vec<String> = vec![];
    for (path, _) in files {
        let mut acc = String::new();
        let mut components: Vec<&str> = path.split('\\').collect();
        components.pop();
        for component in components {
            if !acc.is_empty() {
                acc.push('\\');
            }
            acc.push_str(component);
            if !dirs.contains(&acc) {
                dirs.push(acc.clone());
            }
        }
    }

    let mut raw = vec![];
    for dir in &dirs {
        raw.push(RawEntry {
            crc: crc_of(dir),
            father_crc: father_crc_of(dir),
            flag: FLAG_IS_DIR,
            content: vec![],
            name: gbk(leaf_name(dir)),
            origin_size: 0,
        });
    }
    for (path, data) in files {
        raw.push(RawEntry {
            crc: crc_of(path),
            father_crc: father_crc_of(path),
            flag: FLAG_IS_FILE | FLAG_NOT_COMPRESSED,
            content: data.to_vec(),
            name: gbk(leaf_name(path)),
            origin_size: data.len() as u32,
        });
    }

    let file_num = raw.len() as u32;
    let table_start = HEADER_SIZE;
    let data_start = table_start + file_num * TABLE_ENTRY_SIZE;

    let mut offsets = Vec::with_capacity(raw.len());
    let mut offset = data_start;
    for r in &raw {
        offsets.push(offset);
        offset += r.content.len() as u32 + r.name.len() as u32;
    }
    let package_size = offset;

    let mut buf = vec![];
    buf.write_u32::<LittleEndian>(0x1A545352).unwrap();
    buf.write_u32::<LittleEndian>(1).unwrap();
    buf.write_u32::<LittleEndian>(table_start).unwrap();
    buf.write_u32::<LittleEndian>(data_start).unwrap();
    buf.write_u32::<LittleEndian>(file_num).unwrap();
    buf.write_u32::<LittleEndian>(file_num).unwrap();
    buf.write_u32::<LittleEndian>(1).unwrap();
    buf.write_u32::<LittleEndian>(table_start).unwrap();
    buf.write_u32::<LittleEndian>(file_num).unwrap();
    buf.write_u32::<LittleEndian>(file_num).unwrap();
    buf.write_u32::<LittleEndian>(0).unwrap();
    buf.write_u32::<LittleEndian>(package_size).unwrap();
    for _ in 0..20 {
        buf.write_u32::<LittleEndian>(0).unwrap();
    }

    for (i, r) in raw.iter().enumerate() {
        buf.write_u32::<LittleEndian>(r.crc).unwrap();
        buf.write_u32::<LittleEndian>(r.flag).unwrap();
        buf.write_u32::<LittleEndian>(r.father_crc).unwrap();
        buf.write_u32::<LittleEndian>(offsets[i]).unwrap();
        buf.write_u32::<LittleEndian>(r.content.len() as u32)
            .unwrap();
        buf.write_u32::<LittleEndian>(r.origin_size).unwrap();
        buf.write_u32::<LittleEndian>(r.name.len() as u32).unwrap();
    }

    for r in &raw {
        buf.extend_from_slice(&r.content);
        buf.extend_from_slice(&r.name);
    }

    buf
}

/// Writes [`build_fixture_cpk`]'s bytes to `dir/name`, creating parent
/// directories as needed, and returns the full path.
pub fn write_fixture_cpk(dir: &Path, name: &str, files: &[(&str, &[u8])]) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, build_fixture_cpk(files)).unwrap();
    path
}

/// One planned `Add`/`Replace` change plus its payload bytes, for
/// [`build_fixture_yapatch`].
pub struct FixtureChange {
    pub kind: AssetChangeKind,
    pub target_package: &'static str,
    pub package_internal_path: &'static str,
    pub payload: &'static [u8],
    pub base_entry_hash: Option<ContentHash>,
}

impl FixtureChange {
    pub fn add(
        target_package: &'static str,
        internal_path: &'static str,
        payload: &'static [u8],
    ) -> Self {
        Self {
            kind: AssetChangeKind::Add,
            target_package,
            package_internal_path: internal_path,
            payload,
            base_entry_hash: None,
        }
    }

    pub fn replace(
        target_package: &'static str,
        internal_path: &'static str,
        payload: &'static [u8],
        base_entry_hash: ContentHash,
    ) -> Self {
        Self {
            kind: AssetChangeKind::Replace,
            target_package,
            package_internal_path: internal_path,
            payload,
            base_entry_hash: Some(base_entry_hash),
        }
    }
}

/// Builds a real, fully-verified `.yapatch` at `path` (via
/// `asset_project::patch::publish`, the same atomic write-verify-rename
/// path production authoring tools would use) with one
/// `PackageFingerprint` per distinct `target_package` referenced by
/// `changes` (whole-file hash of `package_bytes_for_fingerprint`, the
/// same convention `crate::fingerprint::package_fingerprint` uses),
/// plus every change in `changes`.
pub fn build_fixture_yapatch(
    path: &Path,
    target_game: &str,
    base_project_version: u32,
    package_fingerprints: &[(&str, ContentHash)],
    changes: &[FixtureChange],
) -> PatchManifest {
    let fingerprints: Vec<PackageFingerprint> = package_fingerprints
        .iter()
        .map(|(target_package, hash)| PackageFingerprint {
            target_package: TargetPackage::new(*target_package).unwrap(),
            base_hash: *hash,
        })
        .collect();

    let entries: Vec<(AssetChange, Vec<u8>)> = changes
        .iter()
        .map(|c| {
            let change = AssetChange::from_payload(
                c.kind,
                TargetPackage::new(c.target_package).unwrap(),
                PackagePath::new(c.package_internal_path).unwrap(),
                c.payload,
                c.base_entry_hash,
                None,
                None,
            );
            (change, c.payload.to_vec())
        })
        .collect();

    publish(
        path,
        target_game,
        base_project_version,
        fingerprints,
        entries,
    )
    .unwrap()
}
