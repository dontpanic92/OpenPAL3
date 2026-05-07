use self::application::OpenPal4ApplicationLoader;
use shared::{config::YaobowConfig, GameType};

pub mod application;

pub fn run_openpal4() {
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    let asset_path = YaobowConfig::load().asset_path_for(GameType::PAL4).to_string();

    #[cfg(target_os = "android")]
    let asset_path = "/sdcard/Games/PAL4".to_string();

    #[cfg(vita)]
    let asset_path = String::new();

    let app = OpenPal4ApplicationLoader::create_application(asset_path, "OpenPAL4");
    app.initialize();
    app.run();
}
