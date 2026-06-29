mod debug_layer;
mod sce_proc_hooks;
pub mod service;

use shared::GameType;

use crate::application::{Pal4AgentBootOptions, boot_for, run_app};

pub use service::Pal3Service;

pub fn run_openpal3() {
    run_openpal3_with_agent(None);
}

/// Boot PAL3 with an optional embedded agent server. `None` keeps
/// the classic flow; `Some(opts)` starts the HTTP listener and
/// pipes synthetic input + pause/step into the `AdventureDirector`.
/// Activated by `yaobow --pal3 --agent-port N`.
pub fn run_openpal3_with_agent(agent: Option<Pal4AgentBootOptions>) {
    let opts = boot_for(GameType::PAL3).with_agent_opts_opt(agent);
    log::info!(
        "initializing OpenPAL3 with asset_path={}",
        opts.asset_path.as_deref().unwrap_or("(empty)")
    );
    run_app(opts);
}

pub fn run_openpal3a() {
    run_openpal3a_with_agent(None);
}

/// Boot PAL3A (shares `Pal3Service` with PAL3). Activated by
/// `yaobow --pal3a [--agent-port N]`.
pub fn run_openpal3a_with_agent(agent: Option<Pal4AgentBootOptions>) {
    let opts = boot_for(GameType::PAL3A).with_agent_opts_opt(agent);
    log::info!(
        "initializing OpenPAL3A with asset_path={}",
        opts.asset_path.as_deref().unwrap_or("(empty)")
    );
    run_app(opts);
}
