//! Game-agnostic agent-server glue shared by every per-game adapter
//! (PAL3, PAL4, …).
//!
//! See the module docs in `bridge.rs` and `launch.rs` for details.
//! Game-specific concerns (VM trace conversion, command dispatch,
//! snapshot building) stay in each `openpalN::agent` module so this
//! crate has no compile-time dependency on any particular VM.

pub mod bridge;
pub mod launch;

pub use bridge::{AgentBridge, DEFAULT_STEP_DT};
pub use launch::{AgentBootOptions, install_global_log_sink, start_agent_server};
