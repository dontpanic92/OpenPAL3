mod application;
mod debug_layer;
mod main_menu_director;
mod sce_proc_hooks;

use application::OpenPal3ApplicationLoader;
use shared::config::YaobowConfig;

pub fn run_openpal3() {
    #[cfg(any(windows, linux, macos))]
    let config = YaobowConfig::load("openpal3.toml", "OPENPAL3");

    #[cfg(android)]
    let config = YaobowConfig {
        asset_path: "/sdcard/Games/PAL3".to_string(),
    };

    #[cfg(vita)]
    let config = YaobowConfig {
        asset_path: "ux0:games/PAL3".to_string(),
    };

    let app = OpenPal3ApplicationLoader::create_application(&config, "OpenPAL3");
    log::info!("initializing with config {:?}", config);
    app.initialize();
    app.run();
}
