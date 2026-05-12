//! Verifies welcome.p7 and settings.p7 parse and load against the same
//! module-provider as production.
const WELCOME_SRC: &str = include_str!("../../../yaobow/yaobow_editor/scripts/welcome.p7");
const SETTINGS_SRC: &str = include_str!("../../../yaobow/yaobow_editor/scripts/settings.p7");

#[test]
fn welcome_script_compiles() {
    let mut runtime = radiance_scripting::ScriptRuntime::new();
    runtime
        .load_source(WELCOME_SRC)
        .expect("welcome.p7 should compile");
}

#[test]
fn settings_script_compiles() {
    let mut runtime = radiance_scripting::ScriptRuntime::new();
    runtime
        .load_source(SETTINGS_SRC)
        .expect("settings.p7 should compile");
}
