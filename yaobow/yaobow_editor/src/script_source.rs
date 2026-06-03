//! Editor p7 script bundle.
//!
//! Each `.p7` file in `scripts/` is a separate protosept module. `main.p7`
//! is the user-main: it imports the sibling modules and exposes the
//! entry-point functions (`init`, `init_main_editor`) that Rust calls.
//!
//! Hosts register the sibling modules with `ScriptHost::add_binding`
//! before calling `ScriptHost::load_source(MAIN_P7)` so the binding
//! provider can resolve their imports at compile time. The
//! [`EDITOR_PACKAGE`] manifest bundles every required IDL binding +
//! sibling module so callers can drive the full registration through
//! [`ScriptPackage::register_bindings`] / [`ScriptPackage::ensure_loaded`].

use radiance_scripting::{ScriptHost, ScriptModule, ScriptPackage};

use crate::editor_bindings::EDITOR_SERVICES_P7;

pub const MAIN_P7: &str = include_str!("../scripts/main.p7");

/// p7 IDL bindings the editor scripts import (codegen-derived).
///
/// Registered before [`SIBLING_MODULES`] so app-owned modules can
/// `import yaobow_editor_services;`.
pub const IDL_BINDINGS: &[ScriptModule] = &[ScriptModule::new(
    "yaobow_editor_services",
    EDITOR_SERVICES_P7,
)];

/// Sibling modules referenced by `main.p7`.
pub const SIBLING_MODULES: &[ScriptModule] = &[
    ScriptModule::new("editor_consts", include_str!("../scripts/editor_consts.p7")),
    ScriptModule::new("icons", include_str!(concat!(env!("OUT_DIR"), "/icons.p7"))),
    ScriptModule::new("theme_menu", include_str!("../scripts/theme_menu.p7")),
    ScriptModule::new("welcome", include_str!("../scripts/welcome.p7")),
    ScriptModule::new("content_tabs", include_str!("../scripts/content_tabs.p7")),
    ScriptModule::new("resource_tree", include_str!("../scripts/resource_tree.p7")),
    ScriptModule::new(
        "resources_panel",
        include_str!("../scripts/resources_panel.p7"),
    ),
    ScriptModule::new("scene_outline", include_str!("../scripts/scene_outline.p7")),
    ScriptModule::new("inspector", include_str!("../scripts/inspector.p7")),
    ScriptModule::new("main_editor", include_str!("../scripts/main_editor.p7")),
];

/// Static manifest for the editor's p7 project. Use
/// [`ScriptPackage::register_bindings`] or
/// [`ScriptPackage::ensure_loaded`] to install it on a `ScriptHost`.
pub const EDITOR_PACKAGE: ScriptPackage = ScriptPackage {
    root_name: "main",
    root_source: MAIN_P7,
    idl_bindings: IDL_BINDINGS,
    modules: SIBLING_MODULES,
};

/// Registers every IDL binding and sibling module with `host`. After
/// this, callers must `host.load_source(MAIN_P7)` to compile the
/// user-main. Bindings survive `ScriptHost::reload`, but a host that
/// fully recreates its `ScriptHost` must call this again.
pub fn register_editor_modules(host: &ScriptHost) {
    EDITOR_PACKAGE
        .validate()
        .expect("editor script package manifest must be valid");
    EDITOR_PACKAGE.register_bindings(host);
}
