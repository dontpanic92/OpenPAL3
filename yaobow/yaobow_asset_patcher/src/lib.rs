//! Yaobow Asset Patcher: a standalone transactional installer for
//! `.ybpatch` asset patches (see `asset_project::patch`) against a
//! PAL3 install.
//!
//! This crate is split into a reusable library and a lightweight
//! p7-lcl GUI binary. The GUI is intentionally thin — all
//! correctness-critical behavior lives in the Rust library.
//!
//! ## Module map
//! - [`environment`]: PAL3 root detection/selection.
//! - [`config`]: minimal read-only `yaobow.toml` reader (last-used
//!   asset path), independent of the `shared` crate.
//! - [`fingerprint`]: whole-package and single-entry content hashing.
//! - [`plan`]: dry-run plan derived from a `.ybpatch`'s manifest,
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
//!   installs and uninstalls.

pub mod bridge;
pub mod config;
pub mod environment;
pub mod error;
pub mod fault;
pub mod fingerprint;
pub mod manager;
pub mod plan;
pub mod replace;
pub mod service;
pub mod startup;
pub mod state;
pub mod transaction;
pub mod validate;

#[cfg(any(test, feature = "test-support"))]
pub mod test_scratch;

#[cfg(any(test, feature = "test-support"))]
pub mod fixtures;

pub use error::{PatcherError, Result};
