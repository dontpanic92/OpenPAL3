//! Compile-only smoke for the PAL3 in-game status HUD + the standalone
//! character-status (状态) menu it delegates to. Loading `status_ui.p7`
//! transitively imports `state_menu.p7`, so this one load exercises both
//! against the production script VFS (so `import radiance_scripting.ui;`
//! and `import yaobow.openpal3.state_menu;` resolve).

use radiance_scripting::ScriptHost;
use yaobow_lib::script_source::install_script_assets;

#[test]
fn pal3_status_ui_and_state_menu_compile() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source_from_path("/yaobow/openpal3/status_ui.p7")
        .expect("openpal3/status_ui.p7 (+ state_menu.p7) must compile");
}
