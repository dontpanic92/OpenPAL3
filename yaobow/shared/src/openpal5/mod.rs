pub mod asset_loader;
#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/shared_openpal5_comdef.rs"));
}
pub mod context;
pub mod scene;

/// p7 binding source generated from `openpal5.idl`. Hosts must
/// register this with `ScriptHost::add_binding("openpal5", ...)`
/// before loading any script that `import openpal5;`.
pub const OPENPAL5_P7: &str = include_str!(concat!(env!("OUT_DIR"), "/shared_openpal5.p7"));
