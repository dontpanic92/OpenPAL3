mod debug_layer;
mod main_menu_director;
mod sce_proc_hooks;
pub mod service;

use shared::GameType;

use crate::application::{boot_for, run_app};

pub use service::Pal3Service;

pub fn run_openpal3() {
    let opts = boot_for(GameType::PAL3);
    log::info!(
        "initializing OpenPAL3 with asset_path={}",
        opts.asset_path.as_deref().unwrap_or("(empty)")
    );
    run_app(opts);
}
