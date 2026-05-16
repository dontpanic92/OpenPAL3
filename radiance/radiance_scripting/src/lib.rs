#[macro_use]
pub mod comdef {
    #[macro_use]
    pub mod scripting {
        include!(concat!(env!("OUT_DIR"), "/scripting_comdef.rs"));
    }

    #[macro_use]
    pub mod services {
        include!(concat!(env!("OUT_DIR"), "/services_comdef.rs"));
    }

    #[macro_use]
    pub mod immediate_director {
        include!(concat!(env!("OUT_DIR"), "/immediate_director_comdef.rs"));
    }
}

pub mod proxies;
pub mod runtime;
pub mod services;

pub use proxies::{wrap_director, wrap_im_director, ImguiImmediateDirectorPump};
pub use runtime::{RuntimeServices, ScriptDirectorHandle, ScriptHost};
pub use services::HostContext;

// Re-export crosscom-protosept primitives that editor bootstrap code
// uses (RuntimeHandle, RuntimeAccess, with_services). Keeps the
// dependency surface for `yaobow_editor` minimal — it can pull
// everything it needs from this crate.
pub use crosscom_protosept::{
    register_proto_ccw, with_services, ArgKind, MethodSpec, ProtoSpec, RetKind, RuntimeAccess,
    RuntimeHandle,
};
