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
pub mod script_package;
pub mod services;

pub use proxies::{ImguiImmediateDirectorPump, install_imgui_pump, install_imgui_pump_with_cache};
// Auto-generated bridges re-exported under the historical names so
// callers don't need to know where the codegen lives.
pub use runtime::{RuntimeServices, ScriptDirectorHandle, ScriptHost};
pub use script_bridges::radiance::{register_immediate_director_proto, wrap_director};
pub use script_package::{
    HOST_CONTEXT_TYPE_TAG, ScriptModule, ScriptPackage, bootstrap_script_root,
};
pub use services::HostContext;

/// Generic FreeView camera controller, 1:1 port of
/// `radiance::utils::free_view::FreeViewController` into protosept.
/// Register with `ScriptHost::add_binding("freeview", FREEVIEW_P7)`
/// so per-game launch scripts can `import freeview;` and call
/// `freeview.update(camera, input, delta_sec)` each frame from a
/// `struct[radiance.IDirector]` they own.
pub const FREEVIEW_P7: &str = include_str!("../scripts/freeview.p7");

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
