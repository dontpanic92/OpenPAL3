//! Compile-only smoke for `actor_controller.p7`.
//!
//! Asserts the script source parses against the auto-generated
//! `shared.openpal4` binding by routing the compile through the
//! VFS-backed module provider. Full behavioral coverage (drive
//! `on_updating` against a real scene) belongs to the integration
//! test added by the engine-wiring phase.

use radiance_scripting::ScriptHost;

#[test]
fn actor_controller_p7_compiles() {
    let assets = radiance::asset::AssetManager::new();
    radiance_scripting::mount_engine_bindings(&assets);
    shared::mount_scripts(&assets);

    let host = ScriptHost::new();
    host.set_script_assets(assets);
    host.load_source_from_path("/shared/openpal4/actor_controller.p7")
        .expect("actor_controller.p7 must compile against the shared.openpal4 binding");
}
