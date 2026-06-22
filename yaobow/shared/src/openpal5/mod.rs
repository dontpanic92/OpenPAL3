pub mod asset_loader;
#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/shared_openpal5_comdef.rs"));
}
pub mod scene;
pub mod script;
pub mod service;
