//! Build-time packer for protosept `.p7` script bundles.
//!
//! This crate is consumed by build scripts only — there is no runtime
//! surface.
//!
//! * Default features: nothing public. The crate exists so build
//!   scripts can `script-package = { …, features = ["build"] }`.
//!
//! * **`build` feature**: exposes [`PackInput`], [`ExtraFile`],
//!   [`pack`], and [`rewrite_imports`], plus a slim vendored
//!   `YpkWriter` matching the wire format produced by
//!   `radiance::asset::ypk::YpkWriter`. Build scripts opt into this
//!   feature so the heavy `radiance` crate stays out of `build.rs`
//!   dependency graphs.
//!
//! # Layout of the produced ypk
//!
//! A ypk produced by [`pack`] contains exactly the walked `.p7`
//! files at their `scripts_dir`-relative paths (forward-slash
//! normalised), plus every [`ExtraFile`] appended at its declared
//! `virtual_entry`. There is no manifest — readers identify modules
//! purely by path, mapping `import a.b.c;` → `/a/b/c.p7` on a
//! mounted `radiance::asset::AssetManager` (see
//! `radiance_scripting::script_vfs::ScriptVfsProvider`).

#[cfg(feature = "build")]
pub use build::{ExtraFile, PackInput, pack};

#[cfg(feature = "build")]
mod build;
