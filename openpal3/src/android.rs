mod application;
mod debug_layer;
mod main_menu_director;
mod sce_proc_hooks;

use application::OpenPal3Application;
use opengb::config::OpenGbConfig;

#[cfg_attr(target_os = "android", ndk_glue::main(backtrace = "on"))]
pub fn android_entry() {
    let config = OpenGbConfig {
        asset_path: "/sdcard/Games/PAL3".to_string(),
    };
    let mut app = OpenPal3Application::create(&config, "OpenPAL3");
    println!("initializing with config {:?}", config);
    app.initialize();
    app.run();
}
