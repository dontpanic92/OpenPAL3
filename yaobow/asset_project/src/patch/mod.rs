//! `.yapatch`: a `.ypk`-based delta-pack format.
//!
//! A `.yapatch` file is a regular `.ypk` archive (see
//! `radiance::asset::ypk`) that always carries three kinds of entries:
//!
//! - `manifest.json` — a serialized [`PatchManifest`] describing the
//!   target game, expected base-package fingerprints, and every
//!   [`crate::manifest::AssetChange`] the patch carries.
//! - `manifest.hash` — the hex [`crate::hash::ContentHash`] of the raw
//!   `manifest.json` bytes, checked *before* the JSON is even parsed so
//!   a corrupted manifest is caught with a clear error instead of a
//!   confusing deserialization failure.
//! - `payload/<target_package>/<package_internal_path>` — one entry
//!   per change, holding the asset's converted bytes. Each payload's
//!   hash is checked against the corresponding
//!   `AssetChange::payload.content_hash` on read.
//!
//! Reusing the `.ypk` container means `.yapatch` gets zstd compression,
//! path-hash lookup, and a battle-tested reader/writer for free instead
//! of a bespoke wire format.
//!
//! Publishing a `.yapatch` (see [`YapatchWriter::finish`] and
//! [`publish`]) is atomic: the archive is built at a sibling temp path,
//! fully closed, re-opened and verified (manifest hash + every payload
//! hash), and only *then* renamed onto the destination path. A reader
//! can never observe a partially-written or corrupt file at the
//! destination, and a failed/interrupted publish never touches it.

mod reader;
mod writer;

pub use reader::YapatchReader;
pub use writer::{YapatchWriter, publish};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::hash::ContentHash;
use crate::manifest::{AssetChange, TargetPackage};

/// Highest `PatchManifest::format_version` this build knows how to
/// read.
pub const YAPATCH_FORMAT_VERSION: u32 = 1;

pub(crate) const YAPATCH_MANIFEST_ENTRY: &str = "manifest.json";
pub(crate) const YAPATCH_MANIFEST_HASH_ENTRY: &str = "manifest.hash";
pub(crate) const YAPATCH_PAYLOAD_PREFIX: &str = "payload/";

/// Expected state of one target package before this patch is applied,
/// so an installer can refuse to apply a patch built against a
/// different base than what's actually installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFingerprint {
    pub target_package: TargetPackage,
    /// Hash identifying the expected pre-patch state of
    /// `target_package` (e.g. a hash over its index/manifest, or over
    /// its full contents — the exact derivation is up to the caller
    /// building the patch; this crate only stores and compares it).
    pub base_hash: ContentHash,
}

/// Describes the set of [`AssetChange`]s carried by one `.yapatch`
/// file, plus enough context (target game, base package fingerprints)
/// for an installer to validate it's being applied to the right place.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchManifest {
    pub format_version: u32,
    pub patch_id: Uuid,
    /// Unix seconds when the patch was packed.
    pub created_at: u64,
    /// Game/config key this patch targets, matching
    /// `ProjectManifest::target_game` (e.g. `"pal3"`, `"pal4"`).
    pub target_game: String,
    /// The `ProjectManifest::version` the patch was produced against,
    /// so an installer can detect that the target install has drifted
    /// from the schema the patch expects.
    pub base_project_version: u32,
    /// Expected pre-patch fingerprints of every target package this
    /// patch touches.
    pub package_fingerprints: Vec<PackageFingerprint>,
    pub changes: Vec<AssetChange>,
}

impl PatchManifest {
    pub fn fingerprint_for(&self, target_package: &TargetPackage) -> Option<&PackageFingerprint> {
        self.package_fingerprints
            .iter()
            .find(|f| &f.target_package == target_package)
    }
}

/// Builds the `payload/<target_package>/<package_internal_path>` entry
/// name for one change. Namespacing by `target_package` keeps entries
/// unambiguous even if two different target packages happen to share
/// the same internal path.
pub(crate) fn payload_entry_name(
    target_package: &TargetPackage,
    package_internal_path: &crate::manifest::PackagePath,
) -> String {
    format!(
        "{YAPATCH_PAYLOAD_PREFIX}{}/{}",
        target_package.as_str(),
        package_internal_path.as_str()
    )
}
