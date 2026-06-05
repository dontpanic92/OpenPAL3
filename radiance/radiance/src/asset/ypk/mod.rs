//! `.ypk` — yaobow's own asset bundle format.
//!
//! Self-contained, ZSTD-compressed, xxh3-hashed archive used by
//! `tools/repacker` (writer) and by `packfs::init_virtual_fs` (mounter).
//! Has no game-specific reverse-engineering, which is why it lives in
//! `radiance` rather than `packfs` — engine consumers can ship `.ypk`
//! bundles without pulling in the game-specific format crates.

mod ypk_archive;
mod ypk_fs;

pub use ypk_archive::{YpkArchive, YpkWriter};
pub use ypk_fs::YpkFs;
