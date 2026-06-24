//! Compile-only smoke for the declarative `openpal4/start_menu.p7`, which
//! is migrated onto the `radiance_scripting.ui` widget framework. Mounts
//! the real script asset bundle (engine bindings + radiance_scripting +
//! shared + yaobow, so `import radiance_scripting.ui;` resolves) and
//! loads the script through the production VFS module provider.

use radiance_scripting::ScriptHost;
use yaobow_lib::script_source::install_script_assets;

#[test]
fn pal4_start_menu_compiles_on_ui_framework() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source_from_path("/yaobow/openpal4/start_menu.p7")
        .expect("openpal4/start_menu.p7 must compile against radiance_scripting.ui");
}
