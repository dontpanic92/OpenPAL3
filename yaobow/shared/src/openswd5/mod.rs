pub mod asset_loader;
#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/shared_openswd5_comdef.rs"));
}
pub mod director;
pub mod scene;
pub mod scripting;
pub mod service;

/// p7 binding source generated from `openswd5.idl`. Hosts must
/// register this with `ScriptHost::add_binding("openswd5", ...)`
/// before loading any script that `import openswd5;`.
pub const SWD5_P7: &str = include_str!(concat!(env!("OUT_DIR"), "/shared_openswd5.p7"));
