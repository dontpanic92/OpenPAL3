//! Script-package manifest type.
//!
//! An [`OwnedScriptPackage`] is a runtime description of a p7 project
//! loaded by the host: a single root source file plus the IDL-derived
//! bindings and app-owned sibling modules that the root imports.
//!
//! Each consuming crate (`yaobow`, `yaobow_editor`, `shared`,
//! `radiance_scripting`) packs its `scripts/` directory at build time
//! into a `<crate>.ypk` (see the `script-package` crate). At runtime
//! the bytes are `include_bytes!`-ed and decoded into an
//! `OwnedScriptPackage` via [`OwnedScriptPackage::from_ypk_bytes`].
//! Composite packages (e.g. `yaobow::package()`) glue several decoded
//! packages together with [`OwnedScriptPackage::merge`].
//!
//! # Lifecycle
//!
//! ```ignore
//! let pkg = OwnedScriptPackage::from_ypk_bytes(BYTES)?;
//!
//! pkg.validate().expect("manifest is well-formed");
//! pkg.register_bindings(&host);
//! if let Some(root) = pkg.root_source.as_deref() {
//!     host.load_source(root)?;
//! }
//! ```
//!
//! Or all-in-one (idempotent — does nothing if the host already has
//! the root loaded under `loaded_sentinel`):
//!
//! ```ignore
//! pkg.ensure_loaded(&host, "init")?;
//! ```

use crosscom::{ComInterface, ComRc};
use crosscom_protosept::HostError;
use p7::interpreter::context::Data;
use script_package::{PACKAGE_MANIFEST_ENTRY, PackageManifest};
use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex};

use crate::runtime::ScriptHost;

/// `type_tag` for the canonical `IHostContext` foreign-box used by
/// `[script_app_root]` packages whose `init` takes the radiance
/// host context. App-specific contexts (e.g. `IEditorHostContext`)
/// have their own tag — pass it explicitly to
/// [`bootstrap_script_root`].
///
/// Hard-coded here for now; step (5) of the script-source refactor
/// (per-interface `intern_<i>_arg` codegen) will generate this
/// alongside every scriptable interface so the magic string goes
/// away.
pub const HOST_CONTEXT_TYPE_TAG: &str = "radiance_scripting.comdef.services.IHostContext";

/// Owned script package backed by data decoded out of a `.ypk` script
/// bundle (produced by the `script-package` crate's build-time
/// packer).
///
/// Module bundles (e.g. `shared::script_bundle()`,
/// `radiance_scripting::script_bundle()`) leave `root_name` /
/// `root_source` as `None` and just contribute IDL bindings + sibling
/// modules. Top-level apps (`yaobow::package()`,
/// `yaobow_editor::package()`) carry a root.
#[derive(Clone, Debug)]
pub struct OwnedScriptModule {
    pub name: String,
    pub source: Arc<str>,
}

impl OwnedScriptModule {
    pub fn new(name: impl Into<String>, source: impl Into<Arc<str>>) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
        }
    }
}

/// See [`OwnedScriptModule`] for rationale on the owned representation.
#[derive(Clone, Debug, Default)]
pub struct OwnedScriptPackage {
    pub root_name: Option<String>,
    pub root_source: Option<Arc<str>>,
    pub idl_bindings: Vec<OwnedScriptModule>,
    pub modules: Vec<OwnedScriptModule>,
}

impl OwnedScriptPackage {
    /// Decode an `include_bytes!`-ed `.ypk` produced by
    /// `script_package::pack`.
    ///
    /// Uses the canonical engine ypk reader
    /// (`radiance::asset::ypk::YpkArchive`) so the wire format is
    /// shared with the writers that live in `radiance::asset::ypk` and
    /// the vendored `script-package::build` writer. The
    /// `script_package_round_trips_via_radiance_ypk_archive` test
    /// guards against drift.
    pub fn from_ypk_bytes(bytes: &'static [u8]) -> anyhow::Result<Arc<Self>> {
        // YpkArchive holds an `Arc<Mutex<dyn SeekRead + Send + Sync>>`
        // over the reader. `Cursor<&'static [u8]>` satisfies that
        // bound, so we wrap once and let the archive serve random
        // access from the in-memory blob.
        let cursor = Cursor::new(bytes);
        let reader: Arc<Mutex<dyn radiance::asset::seek_traits::SeekRead + Send + Sync>> =
            Arc::new(Mutex::new(cursor));
        let mut archive = radiance::asset::ypk::YpkArchive::load(reader)?;

        let manifest_bytes = read_entry(&mut archive, PACKAGE_MANIFEST_ENTRY)?;
        let manifest: PackageManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|e| anyhow::anyhow!("failed to parse __package.json: {e}"))?;

        let root_source = match manifest.root_entry.as_deref() {
            Some(entry) => Some(read_entry_as_str(&mut archive, entry)?),
            None => None,
        };

        let idl_bindings = manifest
            .idl_bindings
            .iter()
            .map(|m| {
                Ok::<_, anyhow::Error>(OwnedScriptModule {
                    name: m.name.clone(),
                    source: read_entry_as_str(&mut archive, &m.entry)?,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let modules = manifest
            .modules
            .iter()
            .map(|m| {
                Ok::<_, anyhow::Error>(OwnedScriptModule {
                    name: m.name.clone(),
                    source: read_entry_as_str(&mut archive, &m.entry)?,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Arc::new(Self {
            root_name: manifest.root_name,
            root_source,
            idl_bindings,
            modules,
        }))
    }

    /// Concatenate several packages plus a free-form list of extra
    /// modules into a single composite. Used by `yaobow::package()` to
    /// fuse its own bundle with the `shared` and `radiance_scripting`
    /// module-bundles into one `ScriptPackage`-shaped surface.
    ///
    /// At most one package may contribute a root; if more than one is
    /// `Some`, this panics with a clear message — that's a programmer
    /// error and silently picking one would hide misconfiguration.
    ///
    /// Duplicate module names (across any combination of inputs) also
    /// panic — caller is expected to deconflict at the source.
    pub fn merge(packages: &[Arc<Self>], extra_modules: &[OwnedScriptModule]) -> Arc<Self> {
        let mut root_name = None;
        let mut root_source = None;
        let mut idl_bindings: Vec<OwnedScriptModule> = Vec::new();
        let mut modules: Vec<OwnedScriptModule> = Vec::new();

        for pkg in packages {
            if pkg.root_name.is_some() {
                if root_name.is_some() {
                    panic!(
                        "OwnedScriptPackage::merge: multiple packages contributed a root \
                         ({:?} and {:?})",
                        root_name, pkg.root_name
                    );
                }
                root_name = pkg.root_name.clone();
                root_source = pkg.root_source.clone();
            }
            idl_bindings.extend(pkg.idl_bindings.iter().cloned());
            modules.extend(pkg.modules.iter().cloned());
        }
        modules.extend(extra_modules.iter().cloned());

        let merged = Self {
            root_name,
            root_source,
            idl_bindings,
            modules,
        };
        merged.validate().unwrap_or_else(|err| {
            panic!("OwnedScriptPackage::merge produced invalid package: {err}")
        });
        Arc::new(merged)
    }

    /// Validate that the manifest is well-formed:
    ///
    /// * `root_name`/`root_source` must agree (either both set or both unset).
    /// * `root_name` (when present) must be non-empty.
    /// * Every module in `idl_bindings` and `modules` must have a non-empty name.
    /// * No duplicate names within either group, and no duplicates across the two groups.
    pub fn validate(&self) -> Result<(), String> {
        match (self.root_name.as_ref(), self.root_source.as_ref()) {
            (Some(name), Some(_)) if name.is_empty() => {
                return Err("script package root name must not be empty".into());
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err(
                    "script package root_name and root_source must be set together".into(),
                );
            }
            _ => {}
        }

        let groups: [(&str, &[OwnedScriptModule]); 2] = [
            ("idl_bindings", &self.idl_bindings),
            ("modules", &self.modules),
        ];

        for (group_name, group) in groups {
            for (idx, module) in group.iter().enumerate() {
                if module.name.is_empty() {
                    return Err(format!(
                        "script package module in '{group_name}' at index {idx} has empty name"
                    ));
                }
            }
            for i in 0..group.len() {
                for j in (i + 1)..group.len() {
                    if group[i].name == group[j].name {
                        return Err(format!(
                            "duplicate script module '{}' inside '{group_name}'",
                            group[i].name
                        ));
                    }
                }
            }
        }

        for left in &self.idl_bindings {
            for right in &self.modules {
                if left.name == right.name {
                    return Err(format!(
                        "duplicate script module '{}' across idl_bindings and modules",
                        left.name
                    ));
                }
            }
        }

        Ok(())
    }

    /// Register every IDL binding and sibling module on `host` via
    /// [`ScriptHost::add_binding`]. Idempotent on `host`: re-registering
    /// the same name simply replaces the previous source.
    pub fn register_bindings(&self, host: &ScriptHost) {
        for binding in &self.idl_bindings {
            host.add_binding(binding.name.clone(), binding.source.to_string());
        }
        for module in &self.modules {
            host.add_binding(module.name.clone(), module.source.to_string());
        }
    }

    /// Idempotent install: validate, register every binding, then load
    /// `root_source` only if `host` does not already define
    /// `loaded_sentinel`.
    ///
    /// Panics on validation failure — script packages are static enough
    /// that an invalid manifest is a programmer error and silent
    /// recovery would mask misconfiguration.
    ///
    /// Returns `Ok(())` for module-bundle packages (no root), only
    /// registering bindings.
    pub fn ensure_loaded(
        &self,
        host: &ScriptHost,
        loaded_sentinel: &str,
    ) -> Result<(), HostError> {
        self.validate().unwrap_or_else(|err| {
            panic!(
                "script package '{}' manifest invalid: {err}",
                self.root_name.as_deref().unwrap_or("<module-bundle>")
            )
        });
        self.register_bindings(host);
        if let Some(root) = self.root_source.as_deref() {
            if !host.has_function(loaded_sentinel) {
                host.load_source(root)?;
            }
        }
        Ok(())
    }
}

/// Bootstrap a script project: ensure the package is loaded, intern
/// `ctx` as a foreign box, then call `root_fn(ctx_box)` on the host
/// and return the resulting `Data` (typically a `box<IScriptApp>`).
///
/// `ctx_type_tag` is the IDL-derived type tag of `ctx`'s interface
/// (e.g. [`HOST_CONTEXT_TYPE_TAG`] for the canonical host context;
/// app-specific contexts like `IEditorHostContext` pass their own
/// `<crate>.comdef.<module>.<Iface>` tag). The codegen pass in step
/// (4) of the script-source refactor emits typed wrappers that bake
/// this tag in so callers don't see it.
///
/// The returned `Data` is **not** rooted — the caller decides:
///
/// * Reverse-wrap it immediately via the auto-generated `wrap_<i>`
///   helper (the resulting CCW holds its own retention, so the
///   underlying script box stays live for the wrap's lifetime).
/// * Or call [`ScriptHost::root`] to keep it alive long-term (e.g.
///   for engine-cached singletons like `YaobowScriptProject` that
///   invoke methods on the rooted app each frame).
///
/// # Errors
///
/// Propagates errors from [`OwnedScriptPackage::ensure_loaded`],
/// [`ScriptHost::foreign_box`], and the script-side `root_fn` call.
pub fn bootstrap_script_root<I: ComInterface + 'static>(
    host: &ScriptHost,
    package: &OwnedScriptPackage,
    ctx: ComRc<I>,
    ctx_type_tag: &str,
    root_fn: &str,
) -> Result<Data, HostError> {
    package.ensure_loaded(host, root_fn)?;
    let ctx_id = host.intern(ctx);
    let ctx_box = host.foreign_box(ctx_type_tag, ctx_id)?;
    host.call_returning_data(root_fn, vec![ctx_box])
}

fn read_entry(
    archive: &mut radiance::asset::ypk::YpkArchive,
    entry: &str,
) -> anyhow::Result<Vec<u8>> {
    let mut file = archive
        .open(entry)
        .map_err(|e| anyhow::anyhow!("ypk entry {entry:?} not found: {e}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn read_entry_as_str(
    archive: &mut radiance::asset::ypk::YpkArchive,
    entry: &str,
) -> anyhow::Result<Arc<str>> {
    let bytes = read_entry(archive, entry)?;
    let s = String::from_utf8(bytes)
        .map_err(|e| anyhow::anyhow!("ypk entry {entry:?} is not valid UTF-8: {e}"))?;
    Ok(Arc::from(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(name: &str, src: &str) -> OwnedScriptModule {
        OwnedScriptModule::new(name.to_string(), src.to_string())
    }

    fn owned_pkg(
        root_name: Option<&str>,
        root_src: Option<&str>,
        idl: Vec<OwnedScriptModule>,
        modules: Vec<OwnedScriptModule>,
    ) -> OwnedScriptPackage {
        OwnedScriptPackage {
            root_name: root_name.map(str::to_string),
            root_source: root_src.map(|s| Arc::<str>::from(s)),
            idl_bindings: idl,
            modules,
        }
    }

    #[test]
    fn valid_package_validates() {
        let pkg = owned_pkg(
            Some("app"),
            Some(""),
            vec![owned("a", "")],
            vec![owned("b", "")],
        );
        pkg.validate().expect("must validate");
    }

    #[test]
    fn module_bundle_without_root_validates() {
        let pkg = owned_pkg(None, None, vec![owned("a", "")], vec![owned("b", "")]);
        pkg.validate().expect("module-bundle (no root) must validate");
    }

    #[test]
    fn root_name_without_source_rejected() {
        let pkg = owned_pkg(Some("app"), None, vec![], vec![]);
        let err = pkg.validate().unwrap_err();
        assert!(err.contains("set together"), "got {err}");
    }

    #[test]
    fn duplicate_inside_group_rejected() {
        let pkg = owned_pkg(None, None, vec![], vec![owned("dup", ""), owned("dup", "")]);
        assert!(pkg.validate().unwrap_err().contains("duplicate"));
    }

    #[test]
    fn duplicate_across_groups_rejected() {
        let pkg = owned_pkg(
            None,
            None,
            vec![owned("shared", "")],
            vec![owned("shared", "")],
        );
        assert!(pkg.validate().unwrap_err().contains("across"));
    }

    #[test]
    fn empty_module_name_rejected() {
        let pkg = owned_pkg(None, None, vec![], vec![owned("", "")]);
        assert!(pkg.validate().unwrap_err().contains("empty name"));
    }

    #[test]
    fn merge_concatenates_and_picks_single_root() {
        let app = Arc::new(owned_pkg(
            Some("app"),
            Some("// root"),
            vec![owned("svc", "// idl svc")],
            vec![owned("title", "// title")],
        ));
        let shared = Arc::new(owned_pkg(
            None,
            None,
            vec![],
            vec![owned("actor", "// actor")],
        ));
        let radiance = Arc::new(owned_pkg(
            None,
            None,
            vec![],
            vec![owned("freeview", "// freeview")],
        ));

        let merged =
            OwnedScriptPackage::merge(&[app, shared, radiance], &[owned("extra", "// extra")]);

        assert_eq!(merged.root_name.as_deref(), Some("app"));
        assert_eq!(
            merged.root_source.as_deref().map(|s| s.to_string()),
            Some("// root".to_string())
        );
        let module_names: Vec<&str> = merged.modules.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(module_names, vec!["title", "actor", "freeview", "extra"]);
        assert_eq!(merged.idl_bindings.len(), 1);
    }

    #[test]
    #[should_panic(expected = "multiple packages contributed a root")]
    fn merge_panics_on_two_roots() {
        let a = Arc::new(owned_pkg(Some("a"), Some(""), vec![], vec![]));
        let b = Arc::new(owned_pkg(Some("b"), Some(""), vec![], vec![]));
        let _ = OwnedScriptPackage::merge(&[a, b], &[]);
    }

    #[test]
    #[should_panic(expected = "invalid package")]
    fn merge_panics_on_duplicate_module() {
        let a = Arc::new(owned_pkg(None, None, vec![], vec![owned("x", "// a")]));
        let b = Arc::new(owned_pkg(None, None, vec![], vec![owned("x", "// b")]));
        let _ = OwnedScriptPackage::merge(&[a, b], &[]);
    }
}
