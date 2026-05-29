use crosscom::ComRc;
use radiance::comdef::IApplication;
use radiance::comdef::IApplicationExt;
use shared::config::YaobowConfig;
use shared::ydirs;

static DEFAULT_IMGUI_INI_CONTENT: &'static str = include_str!("../ini/imgui.ini");

/// The editor's preferred theme on first launch.
const DEFAULT_EDITOR_THEME: &str = "blender_dark";

pub fn init_imgui_ini(app: &ComRc<IApplication>) {
    let config_folder = ydirs::config_dir().join("yaobow_editor");
    let _ = std::fs::create_dir_all(&config_folder);
    let imgui_ini = config_folder.join("imgui.ini");

    if !imgui_ini.exists() {
        app.engine()
            .borrow()
            .ui_manager()
            .imgui_context()
            .context_mut()
            .load_ini_settings(DEFAULT_IMGUI_INI_CONTENT);
    }

    app.engine()
        .borrow()
        .ui_manager()
        .imgui_context()
        .context_mut()
        .set_ini_filename(imgui_ini);
}

/// Apply the editor's persisted theme (or the built-in default) to the live
/// imgui context. If the on-disk config has no theme recorded yet, persist
/// the default so subsequent runs and the in-app switcher see a populated
/// value to start from.
pub fn init_theme(app: &ComRc<IApplication>) {
    let mut cfg = YaobowConfig::load();
    let name = cfg.theme_for("editor");
    let name = if name.is_empty() {
        cfg.set_theme("editor", DEFAULT_EDITOR_THEME.to_string());
        if let Err(e) = cfg.save() {
            log::warn!("failed to persist default editor theme: {}", e);
        }
        DEFAULT_EDITOR_THEME.to_string()
    } else {
        name.to_string()
    };

    app.engine()
        .borrow()
        .ui_manager()
        .imgui_context()
        .apply_theme(&name);
}
