//! Verifies welcome.p7 parses and loads against the same module-provider as production.
const WELCOME_SRC: &str = include_str!("../../../yaobow/yaobow_editor/scripts/welcome.p7");

#[test]
fn welcome_script_compiles() {
    let mut runtime = radiance_scripting::ScriptRuntime::new();
    runtime
        .load_source(WELCOME_SRC)
        .expect("welcome.p7 should compile");
}
