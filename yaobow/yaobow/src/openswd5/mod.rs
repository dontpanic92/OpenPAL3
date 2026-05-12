use self::application::OpenSwd5ApplicationLoader;
use shared::{config::YaobowConfig, GameType};

mod application;

pub fn run_openswd5() {
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    let asset_path = YaobowConfig::load()
        .asset_path_for(GameType::SWD5)
        .to_string();

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    let asset_path = String::new();

    let app = OpenSwd5ApplicationLoader::create_application(asset_path);
    app.initialize();
    app.run();
}
