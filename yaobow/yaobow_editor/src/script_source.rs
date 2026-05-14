//! Editor p7 script bundle.
//!
//! Each `.p7` file in `scripts/` is a separate protosept module. `main.p7`
//! is the user-main: it imports the sibling modules and exposes the
//! entry-point functions (`init`, `init_main_editor`) that Rust calls.
//!
//! Hosts register the sibling modules with `ScriptHost::add_binding`
//! before calling `ScriptHost::load_source(MAIN_P7)` so the binding
//! provider can resolve their imports at compile time.

pub const MAIN_P7: &str = include_str!("../scripts/main.p7");

/// Sibling modules referenced by `main.p7`. The first element is the
/// module path (as it appears in `import` statements), the second is the
/// p7 source.
pub const SIBLING_MODULES: &[(&str, &str)] = &[
    ("editor_consts", include_str!("../scripts/editor_consts.p7")),
    ("welcome", include_str!("../scripts/welcome.p7")),
    ("content_tabs", include_str!("../scripts/content_tabs.p7")),
    ("resource_tree", include_str!("../scripts/resource_tree.p7")),
    ("main_editor", include_str!("../scripts/main_editor.p7")),
];

/// Registers every sibling module with `host` via `add_binding`. After
/// this, callers must `host.load_source(MAIN_P7)` to compile the user-main.
/// Bindings survive `ScriptHost::reload`, but a host that fully recreates
/// its `ScriptHost` must call this again.
pub fn register_editor_modules(host: &radiance_scripting::ScriptHost) {
    for (name, source) in SIBLING_MODULES {
        host.add_binding(*name, *source);
    }
}
