pub mod asset_manager;
#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/shared_openpal3_comdef.rs"));
}
pub mod directors;
pub mod loaders;
pub mod scene;
pub mod states;
pub mod ui;
