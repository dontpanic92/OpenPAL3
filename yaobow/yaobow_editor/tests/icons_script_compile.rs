//! Smoke test for the auto-generated `icons` p7 module.
//!
//! `yaobow_editor/build.rs` reads `lucide.codepoints.json` from
//! `radiance-assets` and emits one `pub const ICON_<UPPER_SNAKE>:
//! string = "\u{...}";` per Lucide glyph into `OUT_DIR/icons.p7`. The
//! module is then registered with `register_editor_modules` so editor
//! scripts can `import icons;` and use the constants inline.
//!
//! This test compiles a tiny user-main that imports the generated
//! module and references known well-known Lucide glyph names. It
//! catches:
//!   - codegen mishaps that produce a syntactically invalid p7 file
//!     (e.g. bad escapes, identifier collisions);
//!   - upstream Lucide renames that drop an icon we care about.

use yaobow_editor::editor_bindings::EDITOR_SERVICES_P7;
use yaobow_editor::script_source::register_editor_modules;

const PROBE_SRC: &str = r#"
import icons;

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
    let runtime = radiance_scripting::ScriptHost::new();
    runtime.add_binding("yaobow_editor_services", EDITOR_SERVICES_P7);
    register_editor_modules(&runtime);
    runtime
        .load_source(PROBE_SRC)
        .expect("generated icons.p7 module should compile and resolve");
}
