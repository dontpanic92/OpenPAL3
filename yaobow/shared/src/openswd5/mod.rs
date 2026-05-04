pub mod asset_loader;
#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/shared_openswd5_comdef.rs"));
}
pub mod director;
pub mod scene;
pub mod scripting;
