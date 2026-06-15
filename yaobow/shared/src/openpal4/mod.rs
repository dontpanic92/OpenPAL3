pub mod asset_loader;
pub mod vm_context;
#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/shared_openpal4_comdef.rs"));

    // Companion bridge for the protosept-authored debug overlay (toggled
    // by tilde at runtime). Kept in its own submodule so the generated
    // `ComObject_*!` macros live in `shared::openpal4::comdef::pal4_debug`,
    // matching the IDL's `module(rust)` declaration.
    #[macro_use]
    pub mod pal4_debug {
        include!(concat!(env!("OUT_DIR"), "/shared_pal4_debug_comdef.rs"));
    }
}
pub mod actor;
pub mod agent;
pub mod director;
pub mod game_context;
pub mod launch;
pub mod modes;
pub mod object_component;
pub mod pal4_debug;
pub mod scene;
pub mod scene_editor_access;
pub mod scripting;
pub mod service;
pub mod session;
pub mod states;
pub mod transition;
pub mod uv_anim;
