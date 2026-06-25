//! Compile-only smoke for the declarative `openpal4/loading_overlay.p7`,
//! migrated onto the `radiance_scripting.ui` widget framework (its
//! bespoke per-draw compositing lives in a screen-local
//! `Pal4LoadingLayout` `gui.Element`). Mounts the real script asset
//! bundle and loads the script through the production VFS module
//! provider so `import radiance_scripting.ui;` resolves.

use radiance_scripting::ScriptHost;
use yaobow_lib::script_source::install_script_assets;

#[test]
fn pal4_loading_overlay_compiles_on_ui_framework() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source_from_path("/yaobow/openpal4/loading_overlay.p7")
        .expect("openpal4/loading_overlay.p7 must compile against radiance_scripting.ui");
}
