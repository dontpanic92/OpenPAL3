mod application;
mod debug_layer;
mod main_menu_director;
mod sce_proc_hooks;

use application::OpenPal3Application;
use opengb::config::OpenGbConfig;

pub fn main() {
    let config = OpenGbConfig::load("openpal3.toml", "OPENPAL3");
    let mut app = OpenPal3Application::create(&config, "OpenPAL3");
    app.initialize();
    app.run();
}
