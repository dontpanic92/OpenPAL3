//! Yaobow Asset Patcher: a standalone transactional installer for
//! `.yapatch` asset patches (see `asset_project::patch`) against a
//! PAL3 install.
//!
//! This crate is split into a reusable library (this file and its
//! modules) and a small imgui GUI binary
//! (`src/bin/yaobow_asset_patcher.rs`) built on top of it. The GUI is
//! intentionally thin — all correctness-critical behavior (dry-run
//! planning, validation, transactional apply/rollback, crash
//! recovery) lives in the library and is covered by the tests under
//! `tests/`.
//!
//! ## Module map
//! - [`environment`]: PAL3 root detection/selection.
//! - [`config`]: minimal read-only `yaobow.toml` reader (last-used
//!   asset path), independent of the `shared` crate.
//! - [`fingerprint`]: whole-package and single-entry content hashing.
//! - [`plan`]: dry-run plan derived from a `.yapatch`'s manifest,
//!   grouped by target package.
//! - [`validate`]: pre-flight validation summary (schema/game/package
//!   existence/fingerprints/base entry hashes/permissions).
//! - [`state`]: fine-grained per-package transaction progress,
//!   persisted alongside backups.
//! - [`fault`]: test-only fault-injection seam for the transaction
//!   engine.
//! - [`transaction`]: the transactional apply/rollback engine itself.
//! - [`replace`]: cross-platform, crash-recoverable single-file
//!   replacement primitive used by [`transaction`]'s swap/restore
//!   steps (this is where the Windows-specific "a bare rename can't
//!   safely replace an existing, possibly-locked file" handling
//!   lives).
//! - [`startup`]: startup detection + recovery of interrupted
//!   installs.
//! - [`file_drop`]: documented stub for a future drag-and-drop hook
//!   (see its module doc for why this isn't wired today).

pub mod config;
pub mod environment;
pub mod error;
pub mod fault;
pub mod file_drop;
pub mod fingerprint;
pub mod plan;
pub mod replace;
pub mod startup;
pub mod state;
pub mod transaction;
pub mod validate;

#[cfg(any(test, feature = "test-support"))]
pub mod test_scratch;

#[cfg(any(test, feature = "test-support"))]
pub mod fixtures;

/// Generated `crosscom`/p7 bindings for [`AssetPatcherUiLayer`], built
/// from `idl/yaobow_asset_patcher.idl` by `build.rs`. See that IDL
/// file's header comment for how it reaches `crosscom/idl/radiance.idl`
/// without this crate needing to modify (or even depend on the module
/// path of) any existing crate's IDL.
#[macro_use]
pub mod comdef {
    #![allow(non_snake_case, non_camel_case_types, unused_imports, clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/yaobow_asset_patcher_comdef.rs"));
}

pub mod ui_layer;

pub use error::{PatcherError, Result};
