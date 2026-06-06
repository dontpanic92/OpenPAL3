#[macro_use]
pub mod comdef {
    // Mirror radiance_scripting's services namespace so cross-crate uses of
    // `radiance_scripting::ComObject_*!` macros (which expand `use crate as
    // radiance_scripting`) can resolve `radiance_scripting::comdef::services::*`
    // through this crate.
    pub mod services {
        pub use radiance_scripting::comdef::services::*;
    }

    // Editor-only foreign service interfaces (PreviewerHub, handles,
    // EditorHostContext). Generated from `yaobow_editor_services.idl`.
    // The matching `.p7` binding is packed into the editor script
    // bundle by `build.rs` and surfaced through the script VFS via
    // `script_source::mount_scripts`.
    #[macro_use]
    pub mod editor_services {
        include!(concat!(
            env!("OUT_DIR"),
            "/yaobow_editor_services_comdef.rs"
        ));
    }
}

pub mod config;
pub mod directors;
pub mod script_source;
pub mod services;

pub use shared::GameType;
