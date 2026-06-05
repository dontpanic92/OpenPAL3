//! `.ypk` types — moved to `radiance::asset::ypk` so the engine can
//! consume them without depending on `packfs`. This module is a
//! source-compatibility re-export shim; new callers should prefer
//! `radiance::asset::ypk` directly.

pub use radiance::asset::ypk::{YpkArchive, YpkFs, YpkWriter};
