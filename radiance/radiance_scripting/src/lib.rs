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
pub mod services;

pub use proxies::{install_imgui_pump, install_imgui_pump_with_cache, ImguiImmediateDirectorPump};
// Auto-generated bridges re-exported under the historical names so
// callers don't need to know where the codegen lives.
pub use runtime::{RuntimeServices, ScriptDirectorHandle, ScriptHost};
pub use script_bridges::radiance::{register_immediate_director_proto, wrap_director};
pub use services::HostContext;

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
    register_proto_ccw, with_services, ArgKind, MethodSpec, ProtoSpec, RetKind, RuntimeAccess,
    RuntimeHandle,
};
