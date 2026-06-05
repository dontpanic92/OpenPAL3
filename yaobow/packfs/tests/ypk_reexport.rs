//! Regression for the `packfs::ypk` -> `radiance::asset::ypk` move.
//!
//! `packfs/src/ypk/mod.rs` is now a one-line re-export shim. This
//! test exercises the full read/write API through `packfs::ypk::*`
//! to make sure the shim keeps `tools/repacker` and any other
//! external consumer of `packfs::ypk::{YpkFs, YpkWriter, YpkArchive}`
//! source-compatible.

use std::{
    fs,
    io::{Read, Seek},
    path::PathBuf,
};

use mini_fs::{MiniFs, StoreExt};
use packfs::ypk::{YpkFs, YpkWriter};

fn unique_tmp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "yaobow_packfs_ypk_{}_{}",
        std::process::id(),
        name
    ))
}

#[test]
fn ypk_reexport_round_trip_via_packfs() {
    let ypk_path = unique_tmp("reexport.ypk");
    let _ = fs::remove_file(&ypk_path);

    {
        let f = fs::File::create(&ypk_path).unwrap();
        // `Box::new(f)` without an explicit `dyn` annotation lets
        // the compiler pick the trait that `YpkWriter::new` expects
        // (radiance's `SeekWrite`, post-relocation). This matches
        // how `tools/repacker/src/pal4.rs` calls it.
        let mut ypk = YpkWriter::new(Box::new(f)).unwrap();
        ypk.write_file("plain/hello.txt", b"hello via packfs").unwrap();
        ypk.write_file("nested/a/b/c.bin", &[10u8, 20, 30, 40]).unwrap();
        ypk.finish().unwrap();
    }

    let mut file = fs::File::open(&ypk_path).unwrap();
    // Confirm bytes survived: the writer rewrote the header to point
    // at the entry list — read should not be zero-length.
    let mut sniff = [0u8; 4];
    file.read_exact(&mut sniff).unwrap();
    assert_eq!(&sniff, b"YPK\x01");
    file.seek(std::io::SeekFrom::Start(0)).unwrap();

    let store = YpkFs::new(&ypk_path).unwrap();
    let vfs = MiniFs::new(false).mount("/", store);

    let mut buf = Vec::new();
    vfs.open("/plain/hello.txt")
        .unwrap()
        .read_to_end(&mut buf)
        .unwrap();
    assert_eq!(buf, b"hello via packfs");

    buf.clear();
    vfs.open("/nested/a/b/c.bin")
        .unwrap()
        .read_to_end(&mut buf)
        .unwrap();
    assert_eq!(buf, vec![10u8, 20, 30, 40]);

    let _ = fs::remove_file(&ypk_path);
}
