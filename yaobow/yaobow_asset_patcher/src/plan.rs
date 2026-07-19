//! Dry-run installation plan: what a `.ybpatch` would do, grouped by
//! target package (`.cpk`), without touching disk.

use asset_project::manifest::{AssetChangeKind, PackagePath, TargetPackage};
use asset_project::patch::PatchManifest;

/// One planned change to a single package entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedChange {
    pub kind: AssetChangeKind,
    pub package_internal_path: PackagePath,
    pub payload_size: u64,
    pub has_base_entry_hash: bool,
}

/// All planned changes for one target package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePlan {
    pub target_package: TargetPackage,
    pub changes: Vec<PlannedChange>,
}

impl PackagePlan {
    pub fn add_count(&self) -> usize {
        self.changes
            .iter()
            .filter(|c| c.kind == AssetChangeKind::Add)
            .count()
    }

    pub fn replace_count(&self) -> usize {
        self.changes
            .iter()
            .filter(|c| c.kind == AssetChangeKind::Replace)
            .count()
    }

    pub fn total_payload_size(&self) -> u64 {
        self.changes.iter().map(|c| c.payload_size).sum()
    }
}

/// The full dry-run plan for a `.ybpatch`: every touched package, in
/// manifest order, each with its own changes in manifest order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PatchPlan {
    pub packages: Vec<PackagePlan>,
}

impl PatchPlan {
    pub fn from_manifest(manifest: &PatchManifest) -> Self {
        let mut packages: Vec<PackagePlan> = Vec::new();

        for change in &manifest.changes {
            let planned = PlannedChange {
                kind: change.kind,
                package_internal_path: change.package_internal_path.clone(),
                payload_size: change.payload.size,
                has_base_entry_hash: change.base_entry_hash.is_some(),
            };

            match packages
                .iter_mut()
                .find(|p| p.target_package == change.target_package)
            {
                Some(package_plan) => package_plan.changes.push(planned),
                None => packages.push(PackagePlan {
                    target_package: change.target_package.clone(),
                    changes: vec![planned],
                }),
            }
        }

        Self { packages }
    }

    pub fn touched_packages(&self) -> impl Iterator<Item = &TargetPackage> {
        self.packages.iter().map(|p| &p.target_package)
    }

    pub fn total_changes(&self) -> usize {
        self.packages.iter().map(|p| p.changes.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asset_project::hash::ContentHash;
    use asset_project::manifest::AssetChange;
    use uuid::Uuid;

    fn change(pkg: &str, path: &str, kind: AssetChangeKind, size: u64) -> AssetChange {
        AssetChange {
            kind,
            target_package: TargetPackage::new(pkg).unwrap(),
            package_internal_path: PackagePath::new(path).unwrap(),
            payload: asset_project::manifest::PayloadRef {
                content_hash: ContentHash::of(path.as_bytes()),
                size,
            },
            base_entry_hash: if kind == AssetChangeKind::Replace {
                Some(ContentHash::of(b"old"))
            } else {
                None
            },
            source: None,
            conversion: None,
        }
    }

    fn manifest(changes: Vec<AssetChange>) -> PatchManifest {
        PatchManifest {
            format_version: 1,
            patch_id: Uuid::new_v4(),
            created_at: 0,
            target_game: "pal3".to_string(),
            base_project_version: 1,
            package_fingerprints: vec![],
            changes,
        }
    }

    #[test]
    fn groups_changes_by_target_package_in_manifest_order() {
        let m = manifest(vec![
            change("scene.cpk", "a.dff", AssetChangeKind::Add, 10),
            change("basedata/basedata.cpk", "b.tga", AssetChangeKind::Add, 20),
            change("scene.cpk", "c.dff", AssetChangeKind::Replace, 30),
        ]);

        let plan = PatchPlan::from_manifest(&m);
        assert_eq!(plan.packages.len(), 2);
        assert_eq!(plan.packages[0].target_package.as_str(), "scene.cpk");
        assert_eq!(plan.packages[0].changes.len(), 2);
        assert_eq!(plan.packages[0].add_count(), 1);
        assert_eq!(plan.packages[0].replace_count(), 1);
        assert_eq!(plan.packages[0].total_payload_size(), 40);

        assert_eq!(
            plan.packages[1].target_package.as_str(),
            "basedata/basedata.cpk"
        );
        assert_eq!(plan.total_changes(), 3);
    }

    #[test]
    fn empty_manifest_yields_empty_plan() {
        let m = manifest(vec![]);
        let plan = PatchPlan::from_manifest(&m);
        assert!(plan.is_empty());
        assert_eq!(plan.total_changes(), 0);
    }
}
