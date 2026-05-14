#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/yaobow_editor_comdef.rs"));

    // Mirror radiance_scripting's services namespace so cross-crate uses of
    // `radiance_scripting::ComObject_*!` macros (which expand `use crate as
    // radiance_scripting`) can resolve `radiance_scripting::comdef::services::*`
    // through this crate.
    pub mod services {
        pub use radiance_scripting::comdef::services::*;
    }

    // Editor-only foreign service interfaces (PreviewerHub, handles,
    // EditorHostContext). Generated from `yaobow_editor_services.idl` and
    // exposed to scripts via the matching `.p7` binding (see
    // `editor_bindings::EDITOR_SERVICES_P7`).
    #[macro_use]
    pub mod editor_services {
        include!(concat!(env!("OUT_DIR"), "/yaobow_editor_services_comdef.rs"));
    }
}

pub mod editor_bindings {
    /// p7 binding source for `yaobow_editor_services.idl`. Register it with
    /// `ScriptHost::add_binding("yaobow_editor_services", EDITOR_SERVICES_P7)`
    /// before loading any editor script that imports this module.
    pub const EDITOR_SERVICES_P7: &str =
        include_str!(concat!(env!("OUT_DIR"), "/yaobow_editor_services.p7"));
}

pub mod config;
pub mod directors;
pub mod script_source;
pub mod services;

pub use shared::GameType;
