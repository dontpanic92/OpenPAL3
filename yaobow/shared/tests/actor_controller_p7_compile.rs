//! Compile-only smoke for `actor_controller.p7`.
//!
//! Asserts the script source parses against the auto-generated
//! `openpal4` p7 binding. Full behavioral coverage (drive
//! `on_updating` against a real scene) belongs to the integration
//! test added by the engine-wiring phase.

use radiance_scripting::ScriptHost;

#[test]
fn actor_controller_p7_compiles() {
    let host = ScriptHost::new();
    let bundle = shared::script_bundle();
    // Register `openpal4` (and every other shared IDL binding).
    bundle.register_bindings(&host);
    let actor_controller_src = bundle
        .modules
        .iter()
        .find(|m| m.name == "actor_controller")
        .map(|m| m.source.clone())
        .expect("actor_controller module must be present in shared bundle");
    host.load_source(&actor_controller_src)
        .expect("actor_controller.p7 must compile against the openpal4 binding");
}
