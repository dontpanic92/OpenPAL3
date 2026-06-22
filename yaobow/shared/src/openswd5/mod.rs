pub mod asset_loader;
pub mod agent;
#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/shared_openswd5_comdef.rs"));
}
pub mod director;
pub mod scene;
pub mod scripting;
pub mod service;
