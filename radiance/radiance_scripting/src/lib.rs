#[macro_use]
pub mod comdef {
    #[macro_use]
    pub mod services {
        include!(concat!(env!("OUT_DIR"), "/services_comdef.rs"));
    }
}

/// Auto-generated script bridges from `[protosept(scriptable)]` IDLs.
/// Each submodule exposes `register_<i>_proto()` and `wrap_<i>()`.
pub mod script_bridges {
    pub mod crosscom {
        include!(concat!(env!("OUT_DIR"), "/crosscom_bridge.rs"));
    }
    pub mod radiance {
        include!(concat!(env!("OUT_DIR"), "/radiance_bridge.rs"));
    }
    pub mod scripting_services {
        include!(concat!(env!("OUT_DIR"), "/scripting_services_bridge.rs"));
    }
}

pub mod proxies;
pub mod runtime;
pub mod script_vfs;
pub mod services;

pub use proxies::{ImguiImmediateDirectorPump, install_imgui_pump, install_imgui_pump_with_cache};
// Auto-generated bridges re-exported under the historical names so
// callers don't need to know where the codegen lives.
pub use runtime::{RuntimeServices, ScriptDirectorHandle, ScriptHost};
pub use script_bridges::radiance::{register_immediate_director_proto, wrap_director};
pub use script_vfs::{HOST_CONTEXT_TYPE_TAG, ScriptVfsProvider, bootstrap_script_root_from_path};
pub use services::HostContext;

/// In-binary `.ypk` of codegen engine bindings (`crosscom.p7`,
/// `scripting_services.p7`, `radiance.p7`, `editor.p7`) produced by
/// `build.rs`. Mounted at the script `AssetManager` root by
/// [`mount_engine_bindings`].
const ENGINE_BINDINGS_YPK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/engine_bindings.ypk"));

/// In-binary `.ypk` script bundle produced by `build.rs` from
/// `scripts/`. Currently carries `freeview.p7`. Mounted at
/// `/radiance_scripting/` by [`mount_scripts`].
const SCRIPT_BUNDLE_YPK: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/radiance_scripting.ypk"));

/// Mounts `engine_bindings.ypk` at the script `AssetManager` root
/// (`/`), so `import radiance;`, `import crosscom;`,
/// `import scripting_services;`, and `import editor;` resolve.
/// Should be called once per script `AssetManager`, before any
/// per-crate `mount_scripts` so the engine bindings shadow any
/// accidental crate-level collisions.
pub fn mount_engine_bindings(assets: &radiance::asset::AssetManager) {
    assets
        .mount_ypk_bytes("/", ENGINE_BINDINGS_YPK)
        .expect("engine_bindings.ypk must mount");
}

/// Mounts this crate's `radiance_scripting.ypk` (currently just
/// `freeview.p7`) at `/radiance_scripting/` on the script
/// `AssetManager`, so per-game launchers can
/// `import radiance_scripting.freeview;`.
pub fn mount_scripts(assets: &radiance::asset::AssetManager) {
    assets
        .mount_ypk_bytes("/radiance_scripting", SCRIPT_BUNDLE_YPK)
        .expect("radiance_scripting.ypk must mount");
}

/// Re-export of the radiance-side `wrap_ray_caster` so callers
/// already pulling in `radiance_scripting` don't need to know that
/// the host impl lives in the `radiance` crate.
pub use radiance::scene::wrap_scene_camera;
pub use radiance::utils::ray_casting::wrap_ray_caster;

// Re-export crosscom-protosept primitives that editor bootstrap code
// uses (RuntimeHandle, RuntimeAccess, with_services). Keeps the
// dependency surface for `yaobow_editor` minimal — it can pull
// everything it needs from this crate.
pub use crosscom_protosept::{
    ArgKind, MethodSpec, ProtoSpec, RetKind, RuntimeAccess, RuntimeHandle, register_proto_ccw,
    with_services,
};
