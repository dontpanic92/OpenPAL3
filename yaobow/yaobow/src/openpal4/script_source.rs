//! PAL4 debug overlay script bundle.
//!
//! Mirrors `yaobow_editor::script_source`: the user-main entry-point
//! (`pal4_debug_main.p7`) is `load_source`d and the sibling overlay
//! module is registered with `ScriptHost::add_binding` so the
//! binding provider can resolve its `import` at compile time.

use radiance_scripting::ScriptHost;

pub const PAL4_DEBUG_MAIN_P7: &str = include_str!("../../scripts/pal4_debug_main.p7");

pub const SIBLING_MODULES: &[(&str, &str)] = &[(
    "pal4_debug_overlay",
    include_str!("../../scripts/pal4_debug_overlay.p7"),
)];

/// Registers every sibling module with `host` via `add_binding`.
/// Bindings survive `ScriptHost::reload`.
pub fn register_pal4_debug_modules(host: &ScriptHost) {
    for (name, source) in SIBLING_MODULES {
        host.add_binding(*name, *source);
    }
}
