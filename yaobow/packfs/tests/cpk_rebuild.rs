//! Synthetic PAL3 `.cpk` fixtures + `CpkRebuilder` round-trip tests.
//!
//! The fixture writer below is deliberately independent of
//! `packfs::cpk`'s internal writer code (`cpk_writer.rs`): it re-derives
//! the on-disk layout directly from the documented format (128-byte
//! header, flat 28-byte `CpkTable[]`, then `data` segments each
//! immediately followed by a GBK-encoded name) using only public API
//! (`crc_checksum`) plus raw byte writing. That way these tests exercise
//! the *reader* and `CpkRebuilder` against an independently hand-built
//! byte stream, rather than just round-tripping through our own writer.

use std::{fs, io::Read, path::PathBuf};

use byteorder::{LittleEndian, WriteBytesExt};
use encoding::{EncoderTrap, Encoding};
use packfs::cpk::{CpkArchive, CpkEdit, CpkRebuildError, CpkRebuilder, crc_checksum};

const FLAG_IS_FILE: u32 = 0x1;
const FLAG_IS_DIR: u32 = 0x2;
const FLAG_NOT_COMPRESSED: u32 = 0x10000;
const HEADER_SIZE: u32 = 128;
const TABLE_ENTRY_SIZE: u32 = 28;
const PAL4_DATA_START_MARKER: u32 = 0x00100080;

fn artifact_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("cpk_rebuild_test_artifacts");
    fs::create_dir_all(&dir).unwrap();
    dir
}

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

/// Hand-builds a minimal, valid non-PAL4 `.cpk` containing `files`
/// (backslash-separated relative paths -> content). Ancestor directory
/// entries are synthesized automatically for every unique parent path.
fn build_fixture_cpk(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut dirs: Vec<String> = vec![];
    for (path, _) in files {
        let mut acc = String::new();
        let mut components: Vec<&str> = path.split('\\').collect();
        components.pop(); // drop the file's own leaf name
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
    buf.write_u32::<LittleEndian>(0x1A545352).unwrap(); // label
    buf.write_u32::<LittleEndian>(1).unwrap(); // version
    buf.write_u32::<LittleEndian>(table_start).unwrap();
    buf.write_u32::<LittleEndian>(data_start).unwrap();
    buf.write_u32::<LittleEndian>(file_num).unwrap(); // max_file_num
    buf.write_u32::<LittleEndian>(file_num).unwrap();
    buf.write_u32::<LittleEndian>(1).unwrap(); // is_formatted
    buf.write_u32::<LittleEndian>(table_start).unwrap(); // size_of_header
    buf.write_u32::<LittleEndian>(file_num).unwrap(); // valid_table_num
    buf.write_u32::<LittleEndian>(file_num).unwrap(); // max_table_num
    buf.write_u32::<LittleEndian>(0).unwrap(); // fragment_num
    buf.write_u32::<LittleEndian>(package_size).unwrap();
    for _ in 0..20 {
        buf.write_u32::<LittleEndian>(0).unwrap(); // reserved
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

/// Hand-builds a syntactically valid (but empty) *PAL4* `.cpk`: just a
/// header whose `data_start` is the PAL4 marker, followed by the fixed
/// 0x1000-byte (XXTEA-"encrypted") table buffer `CpkArchive::load`
/// unconditionally reads for PAL4 packages. `file_num = 0`, so the
/// (garbage-decrypted) buffer is never actually parsed into entries.
fn build_fixture_pal4_cpk() -> Vec<u8> {
    let mut buf = vec![];
    buf.write_u32::<LittleEndian>(0x1A545352).unwrap(); // label
    buf.write_u32::<LittleEndian>(1).unwrap(); // version
    buf.write_u32::<LittleEndian>(HEADER_SIZE).unwrap(); // table_start
    buf.write_u32::<LittleEndian>(PAL4_DATA_START_MARKER)
        .unwrap(); // data_start
    buf.write_u32::<LittleEndian>(0).unwrap(); // max_file_num
    buf.write_u32::<LittleEndian>(0).unwrap(); // file_num
    buf.write_u32::<LittleEndian>(1).unwrap(); // is_formatted
    buf.write_u32::<LittleEndian>(HEADER_SIZE).unwrap(); // size_of_header
    buf.write_u32::<LittleEndian>(0).unwrap(); // valid_table_num
    buf.write_u32::<LittleEndian>(0).unwrap(); // max_table_num
    buf.write_u32::<LittleEndian>(0).unwrap(); // fragment_num
    buf.write_u32::<LittleEndian>(HEADER_SIZE + 0x1000).unwrap(); // package_size
    for _ in 0..20 {
        buf.write_u32::<LittleEndian>(0).unwrap(); // reserved
    }
    buf.extend(std::iter::repeat(0u8).take(0x1000));
    buf
}

fn write_fixture(name: &str, bytes: &[u8]) -> PathBuf {
    let path = artifact_dir().join(name);
    fs::write(&path, bytes).unwrap();
    path
}

fn read_all(archive: &mut CpkArchive, path: &str) -> Vec<u8> {
    let mut file = archive.open_str(path).unwrap();
    let mut buf = vec![];
    file.read_to_end(&mut buf).unwrap();
    buf
}

#[test]
fn rebuild_replaces_existing_file_and_preserves_untouched_entries() {
    let source = write_fixture(
        "replace_source.cpk",
        &build_fixture_cpk(&[
            (r"scene\q01\q01.scn", b"original scene data"),
            (r"scene\q01\untouched.txt", b"leave me alone"),
        ]),
    );
    let dest = artifact_dir().join("replace_dest.cpk");

    CpkRebuilder::rebuild(
        &source,
        &dest,
        &[CpkEdit::file(
            r"scene\q01\q01.scn",
            b"replaced scene data".to_vec(),
        )],
    )
    .expect("rebuild should succeed");

    let reader = std::fs::File::open(&dest).unwrap();
    let mut archive = CpkArchive::load(Box::new(std::io::BufReader::new(reader))).unwrap();

    assert_eq!(
        read_all(&mut archive, r"scene\q01\q01.scn"),
        b"replaced scene data"
    );
    // Untouched entries must survive the rebuild byte-for-byte.
    assert_eq!(
        read_all(&mut archive, r"scene\q01\untouched.txt"),
        b"leave me alone"
    );

    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&dest);
}

#[test]
fn rebuild_adds_new_file_and_directory_with_implied_parents() {
    let source = write_fixture(
        "add_source.cpk",
        &build_fixture_cpk(&[(r"readme.txt", b"hello")]),
    );
    let dest = artifact_dir().join("add_dest.cpk");

    CpkRebuilder::rebuild(
        &source,
        &dest,
        &[
            CpkEdit::file(r"newdir\sub\new.txt", b"brand new content".to_vec()),
            CpkEdit::directory(r"emptydir"),
        ],
    )
    .expect("rebuild should succeed");

    let reader = std::fs::File::open(&dest).unwrap();
    let mut archive = CpkArchive::load(Box::new(std::io::BufReader::new(reader))).unwrap();

    assert_eq!(
        read_all(&mut archive, r"newdir\sub\new.txt"),
        b"brand new content"
    );
    // Pre-existing file must still be there.
    assert_eq!(read_all(&mut archive, r"readme.txt"), b"hello");

    let root = archive.build_directory();
    let newdir_children = root.ls("newdir").unwrap();
    assert!(newdir_children.iter().any(|c| c.borrow().name() == "sub"));

    let sub_children = root.ls(std::path::Path::new("newdir").join("sub")).unwrap();
    assert!(sub_children.iter().any(|c| c.borrow().name() == "new.txt"));

    let root_children = root.ls("").unwrap();
    assert!(
        root_children
            .iter()
            .any(|c| c.borrow().name() == "emptydir" && c.borrow().is_dir())
    );

    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&dest);
}

#[test]
fn rebuild_rejects_pal4_packages() {
    let source = write_fixture("pal4_source.cpk", &build_fixture_pal4_cpk());
    let dest = artifact_dir().join("pal4_dest.cpk");

    let result = CpkRebuilder::rebuild(
        &source,
        &dest,
        &[CpkEdit::file("anything.txt", b"data".to_vec())],
    );

    assert!(matches!(result, Err(CpkRebuildError::Pal4NotSupported)));
    assert!(!dest.exists());

    let _ = fs::remove_file(&source);
}
