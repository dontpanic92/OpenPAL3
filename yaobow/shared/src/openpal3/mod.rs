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

/// p7 binding source generated from `openpal3.idl`. Hosts must
/// register this with `ScriptHost::add_binding("openpal3", ...)`
/// before loading any script that `import openpal3;`.
pub const OPENPAL3_P7: &str = include_str!(concat!(env!("OUT_DIR"), "/shared_openpal3.p7"));
