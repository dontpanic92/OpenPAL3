//! Compile-only smoke for `actor_controller.p7`.
//!
//! Asserts the script source parses against the auto-generated
//! `openpal4` p7 binding. Full behavioral coverage (drive
//! `on_updating` against a real scene) belongs to the integration
//! test added by the engine-wiring phase.

use radiance_scripting::ScriptHost;
use shared::openpal4::actor_controller_script::{ACTOR_CONTROLLER_P7, OPENPAL4_P7};

#[test]
fn actor_controller_p7_compiles() {
    let host = ScriptHost::new();
    host.add_binding("openpal4", OPENPAL4_P7);
    host.load_source(ACTOR_CONTROLLER_P7)
        .expect("actor_controller.p7 must compile against the openpal4 binding");
}
