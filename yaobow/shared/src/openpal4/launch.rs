//! PAL4 agent-server boot helpers — re-export shim.
//!
//! The implementation lives in [`crate::agent_common::launch`] now
//! (game-agnostic, also used by PAL3). This module keeps the old
//! import paths working for any external caller still typing
//! `shared::openpal4::launch::...`.

pub use crate::agent_common::launch::{
    AgentBootOptions, install_global_log_sink, start_agent_server,
};
