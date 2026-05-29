//! Startup helpers for picking up the persisted imgui theme.
//!
//! Both the editor and each `yaobow` game runtime entry point call into
//! this module after `app.initialize()` so the on-disk theme choice (see
//! [`crate::config::YaobowConfig::theme_for`]) is applied to the live
//! `ImguiContext`. The editor wires its own variant in
//! `yaobow_editor::config::init_theme` so it can default to a different
//! built-in theme; this module covers the game runtime.

use crosscom::ComRc;
use radiance::comdef::{IApplication, IApplicationExt};

use crate::config::YaobowConfig;

/// Config key the `yaobow` game runtime stores its theme under.
pub const YAOBOW_THEME_KEY: &str = "yaobow";

/// Apply the runtime's persisted theme to the live imgui context, if any.
///
/// Does nothing when the on-disk config has no theme recorded — the
/// built-in default set by `ImguiContext::new` already covers that case
/// and writing back here would change every existing on-disk file.
pub fn apply_runtime_theme(app: &ComRc<IApplication>) {
    let cfg = YaobowConfig::load();
    let name = cfg.theme_for(YAOBOW_THEME_KEY);
    if name.is_empty() {
        return;
    }
    app.engine()
        .borrow()
        .ui_manager()
        .imgui_context()
        .apply_theme(name);
}
