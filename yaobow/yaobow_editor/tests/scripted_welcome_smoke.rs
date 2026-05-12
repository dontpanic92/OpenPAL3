//! Compile-time smoke test for the scripted welcome page wiring.

#[test]
fn scripted_welcome_page_module_compiles() {
    use crosscom::ComRc;
    use radiance::comdef::{IApplication, IDirector};
    use yaobow_editor::directors::ScriptedWelcomePage;

    let _create: fn(ComRc<IApplication>) -> ComRc<IDirector> = ScriptedWelcomePage::create;
}

#[test]
fn welcome_scripts_compile_with_shared_ui_module() {
    let mut runtime = radiance_scripting::ScriptRuntime::new();
    runtime
        .load_source(include_str!("../scripts/welcome.p7"))
        .expect("welcome.p7 compiles");

    let mut runtime = radiance_scripting::ScriptRuntime::new();
    runtime
        .load_source(include_str!("../scripts/settings.p7"))
        .expect("settings.p7 compiles");
}
