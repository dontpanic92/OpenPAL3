pub mod app_context;
pub mod asset_loader;
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
pub mod actor_controller_script;
pub mod director;
pub mod game_context;
pub mod pal4_debug;
pub mod scene;
pub mod scene_editor_access;
pub mod states;
pub mod scripting;
pub mod uv_anim;
