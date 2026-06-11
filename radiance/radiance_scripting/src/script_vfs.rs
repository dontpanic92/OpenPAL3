//! VFS-backed protosept `ModuleProvider` and the boot helpers that
//! drive script projects through it.
//!
//! The script `ScriptHost` carries a dedicated
//! `radiance::asset::AssetManager` (built at boot by
//! `yaobow_lib::install_script_assets` or its test equivalents) that
//! mounts every per-crate script ypk at a fixed VFS path:
//!
//! * `/radiance.p7`, `/crosscom.p7`, `/scripting_services.p7`,
//!   `/editor.p7` — codegen engine bindings (`mount_engine_bindings`).
//! * `/<crate>/...` — per-crate ypks
//!   (`{radiance_scripting,shared,yaobow_editor,yaobow}::mount_scripts`).
//!
//! [`ScriptVfsProvider::load_module`] maps a dotted protosept import
//! path to the corresponding VFS path (`a.b.c` -> `/a/b/c.p7`) and
//! reads the source bytes through the asset manager. The host
//! installs an instance of this provider as part of every
//! `compile_with_provider` call once `set_script_assets` has been
//! used.

use std::path::PathBuf;
use std::rc::Rc;

use crosscom::{ComInterface, ComRc};
use crosscom_protosept::HostError;
use p7::ModuleProvider;
use p7::interpreter::context::Data;
use radiance::asset::AssetManager;

use crate::runtime::ScriptHost;

/// `type_tag` for the canonical `IHostContext` foreign-box used by
/// app-root p7 packages whose `init` takes the radiance
/// host context. App-specific contexts (e.g. `IEditorHostContext`)
/// have their own tag — pass it explicitly to
/// [`bootstrap_script_root_from_path`].
pub const HOST_CONTEXT_TYPE_TAG: &str = "radiance_scripting.comdef.services.IHostContext";

/// A `p7::ModuleProvider` that resolves `import a.b.c;` against a
/// dedicated script `AssetManager`. See the module docstring for the
/// mount layout assumed by [`load_module`](Self::load_module).
#[derive(Clone)]
pub struct ScriptVfsProvider {
    assets: Rc<AssetManager>,
}

impl ScriptVfsProvider {
    pub fn new(assets: Rc<AssetManager>) -> Self {
        Self { assets }
    }
}

impl ModuleProvider for ScriptVfsProvider {
    fn load_module(&self, module_path: &str) -> Option<String> {
        // `a.b.c` -> `/a/b/c.p7`. We build the path component-by-
        // component instead of `replace('.', '/')` so a stray `.`
        // inside a segment can't escape the script root via `..`.
        let mut path = PathBuf::from("/");
        for seg in module_path.split('.') {
            if seg.is_empty() || seg == "." || seg == ".." {
                return None;
            }
            path.push(seg);
        }
        path.set_extension("p7");

        let bytes = self.assets.read_to_end(&path).ok()?;
        String::from_utf8(bytes).ok()
    }

    fn clone_boxed(&self) -> Box<dyn ModuleProvider> {
        Box::new(self.clone())
    }
}

/// Bootstrap a script project through the VFS-resolved root.
///
/// Reads the root source from `root_path` on the script
/// `AssetManager` (installed via
/// [`ScriptHost::set_script_assets`]), compiles it if not already
/// loaded, then interns `ctx` as a foreign box and calls
/// `root_fn(ctx_box)`. Returns the resulting `Data` (typically a
/// `box<IScriptApp>`).
///
/// `ctx_type_tag` is the IDL-derived type tag of `ctx`'s interface
/// (e.g. [`HOST_CONTEXT_TYPE_TAG`] for the canonical host context;
/// app-specific contexts like `IEditorHostContext` pass their own
/// `<crate>.comdef.<module>.<Iface>` tag).
///
/// Typical `root_path` shape: `/yaobow/app.p7`, `/yaobow_editor/main.p7`.
///
/// The returned `Data` is **not** rooted — the caller decides:
///
/// * Reverse-wrap it immediately via the auto-generated `wrap_<i>`
///   helper (the resulting CCW holds its own retention).
/// * Or call [`ScriptHost::root`] to keep it alive long-term (e.g.
///   for engine-cached singletons like `YaobowScriptProject`).
pub fn bootstrap_script_root_from_path<I: ComInterface + 'static>(
    host: &ScriptHost,
    root_path: &str,
    ctx: ComRc<I>,
    ctx_type_tag: &str,
    root_fn: &str,
) -> Result<Data, HostError> {
    if !host.has_function(root_fn) {
        host.load_source_from_path(root_path)?;
    }
    let ctx_id = host.intern(ctx);
    let ctx_box = host.foreign_box(ctx_type_tag, ctx_id)?;
    host.call_returning_data(root_fn, vec![ctx_box])
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::path::PathBuf;

    use radiance::asset::seek_traits::SeekWrite;
    use radiance::asset::ypk::YpkWriter;

    fn unique_tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "script_vfs_provider_{}_{}",
            std::process::id(),
            name
        ))
    }

    /// Pack a tiny in-memory ypk via `YpkWriter` and return its bytes
    /// as `&'static [u8]`. Same dance as `mount_ypk_bytes` callers
    /// use in production.
    fn pack_to_static(test_id: &str, entries: &[(&str, &[u8])]) -> &'static [u8] {
        let path = unique_tmp(&format!("{test_id}.ypk"));
        let _ = fs::remove_file(&path);
        {
            let f = fs::File::create(&path).unwrap();
            let writer: Box<dyn SeekWrite> = Box::new(f);
            let mut ypk = YpkWriter::new(writer).unwrap();
            for (name, bytes) in entries {
                ypk.write_file(name, bytes).unwrap();
            }
            ypk.finish().unwrap();
        }
        let bytes = fs::read(&path).unwrap();
        let _ = fs::remove_file(&path);
        Box::leak(bytes.into_boxed_slice())
    }

    #[test]
    fn resolves_top_level_engine_binding() {
        let bytes = pack_to_static(
            "resolves_top_level_engine_binding",
            &[("radiance.p7", b"// radiance binding")],
        );
        let assets = AssetManager::new();
        assets.mount_ypk_bytes("/", bytes).unwrap();
        let provider = ScriptVfsProvider::new(assets);

        assert_eq!(
            provider.load_module("radiance").as_deref(),
            Some("// radiance binding")
        );
    }

    #[test]
    fn resolves_crate_prefixed_module() {
        let bytes = pack_to_static(
            "resolves_crate_prefixed_module",
            &[("title.p7", b"// title module")],
        );
        let assets = AssetManager::new();
        assets.mount_ypk_bytes("/yaobow", bytes).unwrap();
        let provider = ScriptVfsProvider::new(assets);

        assert_eq!(
            provider.load_module("yaobow.title").as_deref(),
            Some("// title module")
        );
    }

    #[test]
    fn resolves_nested_module_path() {
        let bytes = pack_to_static(
            "resolves_nested_module_path",
            &[("openpal4/actor_controller.p7", b"// actor")],
        );
        let assets = AssetManager::new();
        assets.mount_ypk_bytes("/shared", bytes).unwrap();
        let provider = ScriptVfsProvider::new(assets);

        assert_eq!(
            provider
                .load_module("shared.openpal4.actor_controller")
                .as_deref(),
            Some("// actor")
        );
    }

    #[test]
    fn missing_module_returns_none() {
        let bytes = pack_to_static("missing_module_returns_none", &[("foo.p7", b"// foo")]);
        let assets = AssetManager::new();
        assets.mount_ypk_bytes("/", bytes).unwrap();
        let provider = ScriptVfsProvider::new(assets);

        assert!(provider.load_module("does.not.exist").is_none());
    }

    #[test]
    fn rejects_traversal_segments() {
        let assets = AssetManager::new();
        let provider = ScriptVfsProvider::new(assets);
        assert!(provider.load_module("..").is_none());
        assert!(provider.load_module("foo..bar").is_none());
        assert!(provider.load_module("foo.").is_none());
    }
}
