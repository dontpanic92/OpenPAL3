//! Regression coverage for multi-archive mount fallthrough.
//!
//! Several titles stack more than one package on a single VFS mount
//! point — SWDHC ships four `Texture_*.imd` archives that all mount at
//! `/Texture/Texture`, and only one of them holds any given texture.
//! `MiniFs::open_path` walks its mounts highest-priority first and only
//! continues past a store that answers `NotFound`, so a `PlainFs`
//! archive that flattened "entry missing" into some other
//! `ErrorKind` aborted the whole lookup and made every texture outside
//! the last-mounted archive unresolvable.

use std::io::Read;
use std::path::Path;

use mini_fs::{MiniFs, Store, StoreExt};
use packfs::memory_file::MemoryFile;
use packfs::plain_fs::{PlainArchive, PlainFs};

/// Minimal in-memory `PlainArchive` holding a single named entry.
struct OneEntryArchive {
    name: &'static str,
    content: &'static [u8],
}

impl PlainArchive for OneEntryArchive {
    fn open<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<MemoryFile> {
        if path.as_ref().to_string_lossy() == self.name {
            Ok(MemoryFile::new(std::io::Cursor::new(self.content.to_vec())))
        } else {
            // Exactly what every real `PlainArchive` reports for a miss.
            Err(std::io::Error::from(std::io::ErrorKind::NotFound))?
        }
    }

    fn files(&self) -> Vec<String> {
        vec![self.name.to_string()]
    }
}

fn read_all(vfs: &MiniFs, path: &str) -> std::io::Result<Vec<u8>> {
    let mut file = vfs.open(Path::new(path))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

#[test]
fn a_miss_in_the_top_archive_falls_through_to_the_next() {
    // `late` is mounted last, so it wins priority — mirroring SWDHC's
    // tiny `Texture_20180508.imd` shadowing the 1.5 GB bulk archive.
    let vfs = MiniFs::new(false)
        .mount(
            "/Texture/Texture",
            PlainFs::new(OneEntryArchive {
                name: "bulk.png",
                content: b"bulk-bytes",
            }),
        )
        .mount(
            "/Texture/Texture",
            PlainFs::new(OneEntryArchive {
                name: "late.png",
                content: b"late-bytes",
            }),
        );

    assert_eq!(
        read_all(&vfs, "/Texture/Texture/late.png").expect("top-priority archive must serve"),
        b"late-bytes".to_vec()
    );

    assert_eq!(
        read_all(&vfs, "/Texture/Texture/bulk.png")
            .expect("a miss in the top archive must fall through to the lower-priority one"),
        b"bulk-bytes".to_vec()
    );
}

#[test]
fn a_path_missing_from_every_archive_reports_not_found() {
    let vfs = MiniFs::new(false).mount(
        "/Texture/Texture",
        PlainFs::new(OneEntryArchive {
            name: "bulk.png",
            content: b"bulk-bytes",
        }),
    );

    let error = read_all(&vfs, "/Texture/Texture/nope.png")
        .expect_err("an entry absent from every mount must fail");
    assert_eq!(
        error.kind(),
        std::io::ErrorKind::NotFound,
        "callers (e.g. `open_with_fallback`) branch on NotFound"
    );
}

#[test]
fn plain_fs_preserves_the_not_found_error_kind() {
    let fs = PlainFs::new(OneEntryArchive {
        name: "bulk.png",
        content: b"bulk-bytes",
    });

    let error = match fs.open_path(Path::new("missing.png")) {
        Ok(_) => panic!("missing entry must not open"),
        Err(error) => error,
    };
    assert_eq!(
        error.kind(),
        std::io::ErrorKind::NotFound,
        "PlainFs must not mask NotFound, or MiniFs stops walking its mounts"
    );
}
