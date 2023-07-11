use crosscom::ComRc;
use radiance::comdef::IApplication;
use shared::ydirs;

static DEFAULT_IMGUI_INI_CONTENT: &'static str = include_str!("../ini/imgui.ini");

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
