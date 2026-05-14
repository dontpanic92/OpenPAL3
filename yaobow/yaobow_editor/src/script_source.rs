//! Composes the editor's per-concern p7 scripts into a single user-main
//! module source for `ScriptHost::load_source`.
//!
//! The cross-module proto-struct dispatch limitation requires every
//! `Director` adapter (`HostDirector`, `WelcomeDirector`,
//! `MainEditorDirector`, ...) to live in the same user-main module. We keep
//! the source split across files for navigability but concatenate at load
//! time. The order matters: `main.p7` carries the `import` block and the
//! `HostDirector` adapter; everything else may use forward references to
//! module-level `pub` items.

const MAIN_P7: &str = include_str!("../scripts/main.p7");
const WELCOME_P7: &str = include_str!("../scripts/welcome.p7");
const RESOURCE_TREE_P7: &str = include_str!("../scripts/resource_tree.p7");
const CONTENT_TABS_P7: &str = include_str!("../scripts/content_tabs.p7");
const MAIN_EDITOR_P7: &str = include_str!("../scripts/main_editor.p7");

pub fn compose_editor_script() -> String {
    let mut out = String::new();
    for chunk in [
        MAIN_P7,
        WELCOME_P7,
        RESOURCE_TREE_P7,
        CONTENT_TABS_P7,
        MAIN_EDITOR_P7,
    ] {
        out.push_str(chunk);
        out.push('\n');
    }
    out
}
