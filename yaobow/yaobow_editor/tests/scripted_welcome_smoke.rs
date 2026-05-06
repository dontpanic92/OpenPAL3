//! Compile-time smoke test for the scripted welcome page wiring.

#[test]
fn scripted_welcome_page_module_compiles() {
    use crosscom::ComRc;
    use radiance::comdef::{IApplication, IDirector};
    use yaobow_editor::directors::ScriptedWelcomePage;

    let _create: fn(ComRc<IApplication>) -> ComRc<IDirector> = ScriptedWelcomePage::create;
}
