//! Smoke test for the auto-generated `icons` p7 module.
//!
//! `yaobow_editor/build.rs` reads `lucide.codepoints.json` from
//! `radiance-assets` and emits one `pub const ICON_<UPPER_SNAKE>:
//! string = "\u{...}";` per Lucide glyph into `OUT_DIR/icons.p7`. The
//! module is packed into `yaobow_editor.ypk` (`/yaobow_editor/icons.p7`
//! on the script VFS) so editor scripts can
//! `import yaobow_editor.icons;` and use the constants inline.
//!
//! This test compiles a tiny user-main that imports the generated
//! module and references known well-known Lucide glyph names. It
//! catches:
//!   - codegen mishaps that produce a syntactically invalid p7 file
//!     (e.g. bad escapes, identifier collisions);
//!   - upstream Lucide renames that drop an icon we care about.

const PROBE_SRC: &str = r#"
import yaobow_editor.icons;

pub fn folder_icon() -> string {
    return icons.ICON_FOLDER;
}

pub fn file_icon() -> string {
    return icons.ICON_FILE;
}

pub fn settings_icon() -> string {
    return icons.ICON_SETTINGS;
}
"#;

#[test]
fn icons_module_compiles_and_exposes_known_glyphs() {
    let assets = radiance::asset::AssetManager::new();
    radiance_scripting::mount_engine_bindings(&assets);
    radiance_scripting::mount_scripts(&assets);
    shared::mount_scripts(&assets);
    yaobow_editor::script_source::mount_scripts(&assets);

    let runtime = radiance_scripting::ScriptHost::new();
    runtime.set_script_assets(assets);
    runtime
        .load_source(PROBE_SRC)
        .expect("generated icons.p7 module should compile and resolve");
}
