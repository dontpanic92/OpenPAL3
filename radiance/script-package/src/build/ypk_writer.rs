//! Vendored `.ypk` writer, format-compatible with
//! `radiance::asset::ypk::YpkWriter`.
//!
//! Why a copy lives here: build scripts call into this crate via the
//! `build` feature; if we depended on `radiance::asset::ypk::YpkWriter`
//! directly, every `build.rs` would pull in the full `radiance` crate
//! (Vulkan / imgui / audio), explosively bloating build-script
//! compilation. The wire format is small and stable (see
//! `radiance/radiance/src/asset/ypk/ypk_archive.rs`).
//!
//! IMPORTANT: any change to the on-disk layout here MUST be mirrored in
//! `radiance::asset::ypk::YpkWriter` (and vice versa), because the
//! canonical reader is `radiance::asset::ypk::YpkArchive`. The
//! `script_package_round_trips_via_radiance_ypk_archive` test in
//! `radiance_scripting` exercises both sides to catch drift.

use std::io::{Seek, Write};

use binrw::{BinWrite, binrw};

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
struct YpkEntry {
    hash: u64,
    name_len: u32,

    #[br(count = name_len)]
    name: Vec<u8>,

    offset: u64,
    is_compressed: u32,
    original_size: u32,
    actual_size: u32,
}

pub(crate) struct PackerYpkWriter {
    writer: Box<dyn SeekWrite>,
    entries: Vec<YpkEntry>,
}

impl PackerYpkWriter {
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

        // Same compression policy as the engine writer: zstd everything
        // except `.bik` movies, which are already compressed and would
        // only get bigger.
        let (is_compressed, data) = if name.ends_with(".bik") {
            (false, data.to_vec())
        } else {
            (true, zstd::stream::encode_all(data, 0)?)
        };

        let name = normalize_path(name);
        let name_for_hash = name.to_lowercase();
        let name_bytes = name.as_bytes();

        self.entries.push(YpkEntry {
            hash: xxhash_rust::xxh3::xxh3_64(name_for_hash.as_bytes()),
            name_len: name_bytes.len() as u32,
            name: name_bytes.to_vec(),
            offset,
            is_compressed: is_compressed as u32,
            original_size,
            actual_size: data.len() as u32,
        });

        self.writer.write_all(&data)?;
        Ok(())
    }

    pub fn finish(mut self) -> anyhow::Result<()> {
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
        self.writer.flush()?;
        Ok(())
    }
}

pub(crate) trait SeekWrite: Write + Seek {}
impl<T: Write + Seek> SeekWrite for T {}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .chars()
        .skip_while(|&c| c == '/' || c == '.')
        .collect()
}
