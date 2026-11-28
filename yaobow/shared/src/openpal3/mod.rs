pub mod agent;
pub mod asset_manager;
pub mod audio_mix;
pub mod combat;
#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/shared_openpal3_comdef.rs"));
}
pub mod directors;
pub mod loaders;
mod menu_effects;
pub(crate) mod menu_preview;
pub mod menu_state;
pub mod movie_gallery;
pub mod save_slots;
pub mod scene;
pub mod states;
pub mod ui_atlas;
