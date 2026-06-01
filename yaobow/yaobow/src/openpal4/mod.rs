use self::application::{AgentBootOptions, OpenPal4ApplicationLoader};
use shared::{config::YaobowConfig, GameType};

pub mod application;

pub fn run_openpal4() {
    run_openpal4_with_agent(None);
}

/// Variant of [`run_openpal4`] that boots the embedded agent server
/// alongside the game. `agent: None` is equivalent to the original.
pub fn run_openpal4_with_agent(agent: Option<AgentBootOptions>) {
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    let asset_path = YaobowConfig::load()
        .asset_path_for(GameType::PAL4)
        .to_string();

    #[cfg(target_os = "android")]
    let asset_path = "/sdcard/Games/PAL4".to_string();

    #[cfg(vita)]
    let asset_path = String::new();

    let app = match agent {
        Some(opts) => {
            OpenPal4ApplicationLoader::create_application_with_agent(asset_path, "OpenPAL4", opts)
        }
        None => OpenPal4ApplicationLoader::create_application(asset_path, "OpenPAL4"),
    };
    app.initialize();
    shared::theme_runtime::apply_runtime_theme(&app);
    app.run();
}
