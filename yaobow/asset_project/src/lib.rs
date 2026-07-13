//! Reusable asset-project foundation shared by the editor (authoring
//! asset changes) and the patcher (packing/installing them).
//!
//! This crate has four layers:
//!
//! - [`manifest`] — the versioned [`manifest::ProjectManifest`] (with
//!   `target_game`/`base_asset_root`) that records every tracked
//!   [`manifest::AssetChange`] (add/replace), keyed by
//!   [`manifest::AssetChangeKey`] (`target_package` +
//!   `package_internal_path`), with a [`manifest::PayloadRef`] into
//!   the content store and optional source/conversion metadata.
//!   Persisted atomically via [`atomic::atomic_write`].
//! - [`payload_store`] — content-addressed on-disk storage for
//!   converted asset payload bytes, keyed by [`hash::ContentHash`]
//!   (SHA-256).
//! - [`patch`] — `.yapatch`, a `.ypk`-based delta-pack format
//!   ([`patch::YapatchWriter`] / [`patch::YapatchReader`] /
//!   [`patch::publish`]) that embeds a [`patch::PatchManifest`]
//!   (target game + per-package [`patch::PackageFingerprint`]s) plus
//!   verified payload entries. Publishing is atomic: built at a
//!   sibling temp path, verified, then renamed into place.
//! - [`journal`] — [`journal::InstallationJournal`], an atomically
//!   persisted, ordered record of which `.yapatch` files have been
//!   applied to a target install, so an interrupted install can be
//!   detected and safely retried.
//!
//! All fallible APIs return [`error::Result`] /
//! [`error::AssetProjectError`]. Package-relative paths
//! ([`manifest::TargetPackage`], [`manifest::PackagePath`]) are
//! validated at construction time and reject absolute paths or
//! `.`/`..` traversal components rather than silently normalizing them
//! away.

pub mod atomic;
pub mod error;
pub mod hash;
pub mod journal;
pub mod manifest;
pub mod patch;
pub mod payload_store;

pub use error::{AssetProjectError, Result};
pub use hash::ContentHash;
pub use journal::{InstallStatus, InstallationJournal, JournalEntry};
pub use manifest::{
    AssetChange, AssetChangeKey, AssetChangeKind, AssetSource, ConversionMetadata, PackagePath,
    PayloadRef, ProjectManifest, TargetPackage,
};
pub use patch::{PackageFingerprint, PatchManifest, YapatchReader, YapatchWriter, publish};
pub use payload_store::PayloadStore;
