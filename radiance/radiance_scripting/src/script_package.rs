//! Script-package manifest type.
//!
//! A `ScriptPackage` is a static description of a p7 project loaded by
//! the host: one root source file, plus the IDL-derived bindings and
//! app-owned sibling modules that the root imports.
//!
//! Both `yaobow` and `yaobow_editor` historically defined their own
//! ad-hoc manifest types (`ScriptPackage` / `SIBLING_MODULES`) and
//! their own registration helpers. Lifting the format into this crate
//! keeps the two surfaces in sync and makes the generic
//! `ScriptProjectInstaller` (see [`crate::runtime::ScriptHost`]
//! callers) reusable across binaries.
//!
//! # Lifecycle
//!
//! ```ignore
//! const PACKAGE: ScriptPackage = ScriptPackage { ... };
//!
//! PACKAGE.validate().expect("manifest is well-formed");
//! PACKAGE.register_bindings(&host);
//! host.load_source(PACKAGE.root_source)?;
//! ```
//!
//! Or all-in-one (idempotent — does nothing if the host already has
//! the root loaded under `loaded_sentinel`):
//!
//! ```ignore
//! PACKAGE.ensure_loaded(&host, "init")?;
//! ```

use crosscom::{ComInterface, ComRc};
use crosscom_protosept::HostError;
use p7::interpreter::context::Data;

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

/// A single p7 source module bundled with a [`ScriptPackage`].
///
/// `name` is the import path scripts use (`import <name>;`); `source`
/// is the raw `.p7` text.
#[derive(Clone, Copy, Debug)]
pub struct ScriptModule {
    pub name: &'static str,
    pub source: &'static str,
}

impl ScriptModule {
    pub const fn new(name: &'static str, source: &'static str) -> Self {
        Self { name, source }
    }
}

/// A complete script package: a root source + the bindings it imports.
///
/// * `root_name` — human-readable name of the root source. Used by
///   `validate` to surface a clear error if the manifest is empty;
///   not used for module resolution.
/// * `root_source` — the `.p7` text loaded via
///   [`ScriptHost::load_source`].
/// * `idl_bindings` — codegen-derived p7 bindings (typically emitted
///   from `[protosept(scriptable)]` IDLs by `crosscom-ccidl`).
///   Registered before `modules` so app-owned modules can import them.
/// * `modules` — app-owned `.p7` files that `root_source` imports.
///
/// Both `idl_bindings` and `modules` are registered via
/// [`ScriptHost::add_binding`], which survives
/// [`ScriptHost::reload`]; callers that fully recreate their
/// `ScriptHost` (rather than reloading) must register again.
pub struct ScriptPackage {
    pub root_name: &'static str,
    pub root_source: &'static str,
    pub idl_bindings: &'static [ScriptModule],
    pub modules: &'static [ScriptModule],
}

impl ScriptPackage {
    /// Validate that the manifest is well-formed:
    ///
    /// * `root_name` is non-empty.
    /// * Every module in `idl_bindings` and `modules` has a non-empty
    ///   name.
    /// * No duplicate names within either group, and no duplicates
    ///   across the two groups.
    pub fn validate(&self) -> Result<(), String> {
        if self.root_name.is_empty() {
            return Err("script package root name must not be empty".to_string());
        }

        let groups: [(&str, &[ScriptModule]); 2] = [
            ("idl_bindings", self.idl_bindings),
            ("modules", self.modules),
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

        for left in self.idl_bindings {
            for right in self.modules {
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

    /// Register every IDL binding and every sibling module with `host`
    /// via [`ScriptHost::add_binding`]. Idempotent on `host`:
    /// re-registering the same name simply replaces the previous
    /// source.
    pub fn register_bindings(&self, host: &ScriptHost) {
        for binding in self.idl_bindings {
            host.add_binding(binding.name, binding.source);
        }
        for module in self.modules {
            host.add_binding(module.name, module.source);
        }
    }

    /// Idempotent install: validate, register every binding, then
    /// load `root_source` only if `host` does not already define
    /// `loaded_sentinel`. Use `loaded_sentinel` to name a function
    /// that the root reliably defines (e.g. `"init"`) so re-entrant
    /// installs skip a redundant `load_source`.
    ///
    /// Panics on validation failure — script packages are static and
    /// invalid manifests are programmer errors caught by the
    /// dedicated `package_validates` test in each consuming crate.
    pub fn ensure_loaded(&self, host: &ScriptHost, loaded_sentinel: &str) -> Result<(), HostError> {
        self.validate().unwrap_or_else(|err| {
            panic!(
                "script package '{}' manifest invalid: {err}",
                self.root_name
            )
        });
        self.register_bindings(host);
        if !host.has_function(loaded_sentinel) {
            host.load_source(self.root_source)?;
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
/// Propagates errors from [`ScriptPackage::ensure_loaded`],
/// [`ScriptHost::foreign_box`], and the script-side `root_fn` call.
pub fn bootstrap_script_root<I: ComInterface + 'static>(
    host: &ScriptHost,
    package: &ScriptPackage,
    ctx: ComRc<I>,
    ctx_type_tag: &str,
    root_fn: &str,
) -> Result<Data, HostError> {
    package.ensure_loaded(host, root_fn)?;
    let ctx_id = host.intern(ctx);
    let ctx_box = host.foreign_box(ctx_type_tag, ctx_id)?;
    host.call_returning_data(root_fn, vec![ctx_box])
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &[ScriptModule] = &[ScriptModule::new("a", "")];
    const B: &[ScriptModule] = &[ScriptModule::new("b", "")];
    const DUP_NAMES: &[ScriptModule] =
        &[ScriptModule::new("dup", ""), ScriptModule::new("dup", "")];
    const SHARED: &[ScriptModule] = &[ScriptModule::new("shared", "")];
    const EMPTY_NAME: &[ScriptModule] = &[ScriptModule::new("", "")];

    const VALID: ScriptPackage = ScriptPackage {
        root_name: "app",
        root_source: "",
        idl_bindings: A,
        modules: B,
    };

    #[test]
    fn valid_package_validates() {
        VALID.validate().expect("VALID must validate");
    }

    #[test]
    fn empty_root_name_rejected() {
        let pkg = ScriptPackage {
            root_name: "",
            ..VALID
        };
        assert!(pkg.validate().is_err());
    }

    #[test]
    fn duplicate_inside_group_rejected() {
        let pkg = ScriptPackage {
            modules: DUP_NAMES,
            ..VALID
        };
        assert!(pkg.validate().unwrap_err().contains("duplicate"));
    }

    #[test]
    fn duplicate_across_groups_rejected() {
        let pkg = ScriptPackage {
            idl_bindings: SHARED,
            modules: SHARED,
            ..VALID
        };
        assert!(pkg.validate().unwrap_err().contains("across"));
    }

    #[test]
    fn empty_module_name_rejected() {
        let pkg = ScriptPackage {
            modules: EMPTY_NAME,
            ..VALID
        };
        assert!(pkg.validate().unwrap_err().contains("empty name"));
    }
}
