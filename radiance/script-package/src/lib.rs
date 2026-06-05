//! Script-package manifest types and (optionally) a build-time packer for
//! protosept `.p7` script bundles.
//!
//! This crate ships in two faces:
//!
//! * **Default features** — just the serde manifest types (`PackageManifest`,
//!   `ManifestEntry`) and the well-known `PACKAGE_MANIFEST_ENTRY` constant.
//!   This is what runtime consumers (`radiance_scripting`) pull in to
//!   deserialize `__package.json` out of a ypk produced by the packer.
//!
//! * **`build` feature** — adds [`PackInput`], [`ExtraFile`],
//!   [`ModuleKind`] and [`pack`], plus a slim vendored `YpkWriter` matching
//!   the wire format produced by `radiance::asset::ypk::YpkWriter`. Build
//!   scripts opt into this feature so the heavy `radiance` crate stays
//!   out of `build.rs` dependency graphs.
//!
//! # The ypk layout this crate produces
//!
//! Every script ypk has the same shape:
//!
//! * Entry `__package.json` — the JSON-serialized [`PackageManifest`].
//! * One entry per `.p7` file at the path recorded in the manifest
//!   (`virtual_entry`). Module names in the manifest are the names used
//!   for `import <name>;` at p7 compile time and `ScriptHost::add_binding`
//!   at runtime.

use serde::{Deserialize, Serialize};

/// The fixed ypk entry name where the [`PackageManifest`] lives.
pub const PACKAGE_MANIFEST_ENTRY: &str = "__package.json";

/// Top-level manifest stored at [`PACKAGE_MANIFEST_ENTRY`] inside a script
/// ypk.
///
/// * `root_name` / `root_entry` — set together iff the package has a root
///   source (i.e. a `[script_app_root]`-style entry point that the host
///   `load_source()`s). Module-bundle packages (e.g. `shared`,
///   `radiance_scripting`) leave both `None`.
/// * `idl_bindings` — codegen-derived bindings registered first so
///   sibling modules can import them.
/// * `modules` — app-owned `.p7` files imported by the root.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageManifest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_entry: Option<String>,
    #[serde(default)]
    pub idl_bindings: Vec<ManifestEntry>,
    #[serde(default)]
    pub modules: Vec<ManifestEntry>,
}

/// One module entry inside [`PackageManifest`].
///
/// * `name` — the import name (`import <name>;`).
/// * `entry` — the path inside the ypk where the source bytes live.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub name: String,
    pub entry: String,
}

#[cfg(feature = "build")]
pub use build::{ExtraFile, ModuleKind, PackInput, pack};

#[cfg(feature = "build")]
mod build;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrips_through_json() {
        let manifest = PackageManifest {
            root_name: Some("app".into()),
            root_entry: Some("app.p7".into()),
            idl_bindings: vec![ManifestEntry {
                name: "yaobow_services".into(),
                entry: "idl/yaobow_services.p7".into(),
            }],
            modules: vec![ManifestEntry {
                name: "title".into(),
                entry: "title.p7".into(),
            }],
        };

        let bytes = serde_json::to_vec(&manifest).unwrap();
        let parsed: PackageManifest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn empty_module_bundle_manifest_omits_root_fields() {
        // Module-bundle packages (no root) should serialize without
        // `root_name` / `root_entry` so the on-disk JSON stays minimal.
        let manifest = PackageManifest::default();
        let s = serde_json::to_string(&manifest).unwrap();
        assert!(!s.contains("root_name"), "got {s}");
        assert!(!s.contains("root_entry"), "got {s}");
    }
}
