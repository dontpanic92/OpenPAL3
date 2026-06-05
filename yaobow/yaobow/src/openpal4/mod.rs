use shared::openpal4::launch::AgentBootOptions;
use shared::GameType;

use crate::application::{boot_for, run_app};

/// Re-export the boot options type at the `crate::openpal4::application` path
/// historically used by `yaobow_lib`. The struct itself now lives in
/// `shared::openpal4::launch` (just the agent-boot helpers; PAL4
/// director construction moved to `shared::openpal4::service`).
pub mod application {
    pub use shared::openpal4::launch::AgentBootOptions;
}

pub fn run_openpal4() {
    run_openpal4_with_agent(None);
}

/// Variant of [`run_openpal4`] that boots the embedded agent server
/// alongside the game. `agent: None` is equivalent to the original.
pub fn run_openpal4_with_agent(agent: Option<AgentBootOptions>) {
    run_app(boot_for(GameType::PAL4).with_agent_opts_opt(agent));
}
