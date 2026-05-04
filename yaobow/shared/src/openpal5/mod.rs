pub mod asset_loader;
#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/shared_openpal5_comdef.rs"));
}
pub mod director;
pub mod scene;
