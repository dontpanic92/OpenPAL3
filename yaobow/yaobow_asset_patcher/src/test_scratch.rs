//! Scratch-directory helper for tests, matching the convention used
//! throughout `asset_project`'s own test suite: a directory under this
//! crate's `target/test-tmp/`, uniquely named per test run
//! (pid + nanosecond timestamp) so parallel test runs never collide,
//! with cleanup left to the caller (or a stale `target/` directory,
//! which is fine — it's already gitignored build output).
//!
//! Kept out of `/tmp` deliberately: this repo's own tests (see
//! `asset_project`) always scratch under `CARGO_MANIFEST_DIR/target`
//! rather than the OS temp dir.

use std::path::PathBuf;

pub fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp")
        .join(format!(
            "{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
