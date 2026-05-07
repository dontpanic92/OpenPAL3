mod application;
mod debug_layer;
mod main_menu_director;
mod sce_proc_hooks;

pub use application::OpenPal3ApplicationLoader;
use shared::{config::YaobowConfig, GameType};

pub fn run_openpal3() {
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    let asset_path = {
        let cfg = YaobowConfig::load();
        cfg.asset_path_for(GameType::PAL3).to_string()
    };

    #[cfg(target_os = "android")]
    let asset_path = "/sdcard/Games/PAL3".to_string();

    #[cfg(vita)]
    let asset_path = "ux0:games/PAL3".to_string();

    log::info!("initializing OpenPAL3 with asset_path={}", asset_path);
    let app = OpenPal3ApplicationLoader::create_application(&asset_path, "OpenPAL3");
    app.initialize();
    app.run();
}
