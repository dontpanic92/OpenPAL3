//! Pre-flight validation: schema/game/package-existence/fingerprint/
//! base-entry-hash/permission checks, run before any file on disk is
//! touched. Produces a [`ValidationSummary`] the GUI can render as-is
//! and [`ValidationSummary::is_ok`] gates whether `apply()` may
//! proceed.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use asset_project::hash::ContentHash;
use asset_project::patch::PatchManifest;

use crate::environment::GameRoot;
use crate::fingerprint;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    /// Blocks `apply()`.
    Error,
    /// Surfaced to the user but does not block `apply()` (e.g. a
    /// `Replace` change with no recorded `base_entry_hash` to check).
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub message: String,
}

impl ValidationIssue {
    fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
        }
    }

    fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
        }
    }
}

/// Validation outcome for a single target package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageValidation {
    pub target_package: String,
    pub physical_path: Option<PathBuf>,
    pub exists: bool,
    pub writable: bool,
    pub fingerprint_expected: Option<ContentHash>,
    pub fingerprint_actual: Option<ContentHash>,
    pub fingerprint_matches: Option<bool>,
    pub issues: Vec<ValidationIssue>,
}

impl PackageValidation {
    fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Error)
    }
}

/// Full validation summary for one `.yapatch` against one [`GameRoot`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValidationSummary {
    pub schema_ok: bool,
    pub game_matches: bool,
    pub root_looks_like_pal3: bool,
    pub packages: Vec<PackageValidation>,
    /// Issues not tied to a specific package (schema/game mismatches).
    pub global_issues: Vec<ValidationIssue>,
}

impl ValidationSummary {
    /// Whether `apply()` may proceed: no `Error`-severity issues
    /// anywhere (global or per-package).
    pub fn is_ok(&self) -> bool {
        self.schema_ok
            && self.game_matches
            && self.root_looks_like_pal3
            && !self
                .global_issues
                .iter()
                .any(|i| i.severity == Severity::Error)
            && !self.packages.iter().any(|p| p.has_errors())
    }

    pub fn all_issues(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.global_issues
            .iter()
            .chain(self.packages.iter().flat_map(|p| p.issues.iter()))
    }
}

/// Highest `.yapatch` schema version this installer understands.
/// Mirrors `asset_project::patch::YAPATCH_FORMAT_VERSION`, duplicated
/// here as a local constant so a schema-version issue surfaces as a
/// [`ValidationIssue`] rather than a hard error from `YapatchReader`
/// (which already refuses to open a too-new patch outright, but
/// `validate()` is also called on a manifest object a caller may
/// have obtained some other way, e.g. re-validation after an
/// already-open reader).
const KNOWN_YAPATCH_FORMAT_VERSION: u32 = 1;

/// Validates `manifest` (from an already-opened, hash-verified
/// `.yapatch`) against `root`. Never touches disk beyond `fs::read`
/// (for fingerprints) / `fs::metadata`+a real permission probe (see
/// [`check_writable`]) — no package is modified.
pub fn validate(
    manifest: &PatchManifest,
    root: &GameRoot,
    expected_game: &str,
) -> ValidationSummary {
    let mut summary = ValidationSummary::default();

    summary.schema_ok = manifest.format_version <= KNOWN_YAPATCH_FORMAT_VERSION;
    if !summary.schema_ok {
        summary.global_issues.push(ValidationIssue::error(format!(
            "patch schema version {} is newer than the highest version this installer supports ({})",
            manifest.format_version, KNOWN_YAPATCH_FORMAT_VERSION
        )));
    }

    summary.game_matches = manifest.target_game == expected_game;
    if !summary.game_matches {
        summary.global_issues.push(ValidationIssue::error(format!(
            "patch targets game {:?}, expected {:?}",
            manifest.target_game, expected_game
        )));
    }

    summary.root_looks_like_pal3 = root.looks_like_pal3();
    if !summary.root_looks_like_pal3 {
        summary.global_issues.push(ValidationIssue::error(
            "selected root does not look like a PAL3 install (basedata/basedata.cpk not found)",
        ));
    }

    let plan = crate::plan::PatchPlan::from_manifest(manifest);
    let mut resolved_packages = HashMap::<PathBuf, String>::new();
    for package_plan in &plan.packages {
        let name = package_plan.target_package.as_str();
        if let Some(path) = root.resolve_package_path(name) {
            if let Some(previous) = resolved_packages.insert(path, name.to_string()) {
                summary.global_issues.push(ValidationIssue::error(format!(
                    "target packages {previous:?} and {name:?} resolve to the same physical package"
                )));
            }
        }
        summary
            .packages
            .push(validate_package(manifest, root, package_plan));
    }

    summary
}

fn validate_package(
    manifest: &PatchManifest,
    root: &GameRoot,
    package_plan: &crate::plan::PackagePlan,
) -> PackageValidation {
    let target_package = package_plan.target_package.as_str().to_string();
    let physical_path = root.resolve_package_path(&target_package);
    let exists = physical_path.as_ref().is_some_and(|p| p.exists());

    let mut issues = Vec::new();
    if !exists {
        issues.push(ValidationIssue::error(format!(
            "target package {target_package:?} was not found under the selected root"
        )));
    }

    let writable = physical_path
        .as_ref()
        .map(|p| check_writable(p))
        .unwrap_or(false);
    if exists && !writable {
        issues.push(ValidationIssue::error(format!(
            "target package {target_package:?} is not writable"
        )));
    }

    let fingerprint_expected = manifest
        .fingerprint_for(&package_plan.target_package)
        .map(|f| f.base_hash);

    let fingerprint_actual = if exists {
        physical_path
            .as_ref()
            .and_then(|p| fingerprint::package_fingerprint(p).ok())
    } else {
        None
    };

    let fingerprint_matches = match (fingerprint_expected, fingerprint_actual) {
        (Some(expected), Some(actual)) => {
            let matches = expected == actual;
            if !matches {
                issues.push(ValidationIssue::error(format!(
                    "package fingerprint mismatch for {target_package:?}: expected {}, found {}",
                    expected.to_hex(),
                    actual.to_hex()
                )));
            }
            Some(matches)
        }
        (Some(_), None) => {
            issues.push(ValidationIssue::warning(format!(
                "no fingerprint could be computed for {target_package:?} (package unreadable)"
            )));
            None
        }
        (None, _) => {
            issues.push(ValidationIssue::warning(format!(
                "patch carries no package fingerprint for {target_package:?}; \
                 the pre-patch state of this package cannot be verified"
            )));
            None
        }
    };

    if exists {
        for change in manifest
            .changes
            .iter()
            .filter(|c| c.target_package == package_plan.target_package)
        {
            if let Some(expected_hash) = change.base_entry_hash {
                match physical_path.as_ref().and_then(|p| {
                    fingerprint::base_entry_hash(p, change.package_internal_path.as_str()).ok()
                }) {
                    Some(actual_hash) if actual_hash == expected_hash => {}
                    Some(actual_hash) => {
                        issues.push(ValidationIssue::error(format!(
                            "base entry hash mismatch for {target_package:?}/{:?}: expected {}, found {}",
                            change.package_internal_path.as_str(),
                            expected_hash.to_hex(),
                            actual_hash.to_hex()
                        )));
                    }
                    None => {
                        issues.push(ValidationIssue::error(format!(
                            "entry {:?} declared in patch as a base for replacement was not \
                             found (or could not be read) in {target_package:?}",
                            change.package_internal_path.as_str()
                        )));
                    }
                }
            } else if change.is_replace() {
                issues.push(ValidationIssue::warning(format!(
                    "replace change for {target_package:?}/{:?} carries no base_entry_hash; \
                     the pre-patch entry content cannot be verified",
                    change.package_internal_path.as_str()
                )));
            }
        }
    }

    PackageValidation {
        target_package,
        physical_path,
        exists,
        writable,
        fingerprint_expected,
        fingerprint_actual,
        fingerprint_matches,
        issues,
    }
}

/// Probes whether `path` is actually writable by a real (harmless)
/// write attempt: opening in append mode never truncates/corrupts
/// existing content and is reverted immediately (append-opening
/// doesn't change size unless bytes are actually written, which this
/// never does). More reliable across platforms than inspecting
/// permission bits, which can disagree with actual access due to
/// ACLs, read-only filesystems, or (on Windows) attributes that
/// `std::fs::Permissions` doesn't fully model.
fn check_writable(path: &std::path::Path) -> bool {
    match fs::OpenOptions::new().append(true).open(path) {
        Ok(_) => true,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asset_project::manifest::{AssetChange, AssetChangeKind, PackagePath, TargetPackage};
    use asset_project::patch::PackageFingerprint;
    use uuid::Uuid;

    fn base_manifest() -> PatchManifest {
        PatchManifest {
            format_version: 1,
            patch_id: Uuid::new_v4(),
            created_at: 0,
            target_game: "pal3".to_string(),
            base_project_version: 1,
            package_fingerprints: vec![],
            changes: vec![],
        }
    }

    #[test]
    fn rejects_game_mismatch_and_missing_root() {
        let dir = crate::test_scratch::dir("validate-game-mismatch");
        let root = GameRoot::open(&dir);

        let mut manifest = base_manifest();
        manifest.target_game = "pal4".to_string();

        let summary = validate(&manifest, &root, "pal3");
        assert!(!summary.is_ok());
        assert!(!summary.game_matches);
        assert!(!summary.root_looks_like_pal3);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_future_schema_version() {
        let dir = crate::test_scratch::dir("validate-schema");
        let root = GameRoot::open(&dir);
        let mut manifest = base_manifest();
        manifest.format_version = KNOWN_YAPATCH_FORMAT_VERSION + 1;

        let summary = validate(&manifest, &root, "pal3");
        assert!(!summary.schema_ok);
        assert!(!summary.is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reports_missing_package() {
        let dir = crate::test_scratch::dir("validate-missing-package");
        let root = GameRoot::open(&dir);

        let mut manifest = base_manifest();
        manifest.changes.push(AssetChange::from_payload(
            AssetChangeKind::Add,
            TargetPackage::new("scene.cpk").unwrap(),
            PackagePath::new("a.dff").unwrap(),
            b"payload",
            None,
            None,
            None,
        ));

        let summary = validate(&manifest, &root, "pal3");
        assert_eq!(summary.packages.len(), 1);
        assert!(!summary.packages[0].exists);
        assert!(summary.packages[0].has_errors());
        assert!(!summary.is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fingerprint_mismatch_is_flagged() {
        let dir = crate::test_scratch::dir("validate-fingerprint-mismatch");
        crate::fixtures::write_fixture_cpk(
            &dir.join("basedata"),
            "basedata.cpk",
            &[("marker.txt", b"basedata marker")],
        );
        crate::fixtures::write_fixture_cpk(&dir, "scene.cpk", &[("a.dff", b"actual scene bytes")]);

        let root = GameRoot::open(&dir);
        assert!(root.looks_like_pal3());

        let mut manifest = base_manifest();
        manifest.package_fingerprints.push(PackageFingerprint {
            target_package: TargetPackage::new("scene.cpk").unwrap(),
            base_hash: ContentHash::of(b"different bytes than what's on disk"),
        });
        manifest.changes.push(AssetChange::from_payload(
            AssetChangeKind::Add,
            TargetPackage::new("scene.cpk").unwrap(),
            PackagePath::new("a.dff").unwrap(),
            b"payload",
            None,
            None,
            None,
        ));

        let summary = validate(&manifest, &root, "pal3");
        let pkg = &summary.packages[0];
        assert_eq!(pkg.fingerprint_matches, Some(false));
        assert!(pkg.has_errors());
        assert!(!summary.is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
