mod application;
mod debug_layer;
mod main_menu_director;
mod sce_proc_hooks;

use application::OpenPal3ApplicationLoader;
use opengb::config::OpenGbConfig;

pub fn run_openpal3() {
    let config = OpenGbConfig::load("openpal3.toml", "OPENPAL3");
    let app = OpenPal3ApplicationLoader::create_application(&config, "OpenPAL3");
    app.initialize();
    app.run();
}

#[cfg_attr(target_os = "android", ndk_glue::main(backtrace = "on"))]
pub fn openpal3_android_entry() {
    let config = OpenGbConfig {
        asset_path: "/sdcard/Games/PAL3".to_string(),
    };
    let app = OpenPal3ApplicationLoader::create_application(&config, "OpenPAL3");
    println!("initializing with config {:?}", config);
    app.initialize();
    app.run();
}
