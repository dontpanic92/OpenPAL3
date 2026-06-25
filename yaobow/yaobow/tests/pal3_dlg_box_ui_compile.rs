//! Compile-only smoke for the declarative `openpal3/dlg_box.p7`, migrated
//! onto the `radiance_scripting.ui` widget framework (the 9-slice chrome,
//! wrapped CJK text grid and fade curtain are screen-local `gui.Element`
//! leaves). Mounts the real script asset bundle and loads the script
//! through the production VFS module provider so
//! `import radiance_scripting.ui;` resolves.

use radiance_scripting::ScriptHost;
use yaobow_lib::script_source::install_script_assets;

#[test]
fn pal3_dlg_box_compiles_on_ui_framework() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source_from_path("/yaobow/openpal3/dlg_box.p7")
        .expect("openpal3/dlg_box.p7 must compile against radiance_scripting.ui");
}
