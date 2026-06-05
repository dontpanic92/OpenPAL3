//! Editor p7 script bundle.
//!
//! `scripts/` and the generated bindings (`yaobow_editor_services.p7`,
//! `icons.p7`) are packed at build time by `build.rs` into
//! `OUT_DIR/yaobow_editor.ypk` and exposed through [`package()`]. The
//! resulting [`OwnedScriptPackage`] is `register_bindings`-ed onto a
//! `ScriptHost` and the root `main.p7` is `load_source`-d via
//! `OwnedScriptPackage::ensure_loaded`.

use std::sync::{Arc, OnceLock};

use radiance_scripting::{OwnedScriptPackage, ScriptHost};

/// In-binary `.ypk` produced by `build.rs`. Decoded once on first call
/// to [`package()`].
const EDITOR_YPK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/yaobow_editor.ypk"));

/// Lazily-decoded editor script package. Cached for the lifetime of
/// the binary so all callers share the same `Arc<OwnedScriptPackage>`.
pub fn package() -> Arc<OwnedScriptPackage> {
    static CACHE: OnceLock<Arc<OwnedScriptPackage>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            OwnedScriptPackage::from_ypk_bytes(EDITOR_YPK)
                .expect("yaobow_editor.ypk bundled by build.rs must decode")
        })
        .clone()
}

/// Registers every IDL binding and sibling module from [`package()`]
/// on `host`. After this call, `host.load_source(package().root_source)`
/// can compile `main.p7`. Bindings survive `ScriptHost::reload`, but a
/// host that fully recreates its `ScriptHost` must call this again.
pub fn register_editor_modules(host: &ScriptHost) {
    let pkg = package();
    pkg.validate()
        .expect("editor script package manifest must be valid");
    pkg.register_bindings(host);
}


