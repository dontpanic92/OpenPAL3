//! Yaobow's self-contained ZSTD-compressed asset bundle format.

mod archive;
mod file;
mod fs;

pub use archive::{SeekRead, SeekWrite, YpkArchive, YpkWriter};
pub use fs::YpkFs;
