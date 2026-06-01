//! In-process HTTP+JSON agent surface for OpenPAL games.
//!
//! Provides a small, transport-agnostic facade an external AI agent
//! (or test driver) can call to observe and drive a running game. The
//! crate ships:
//!
//! * [`protocol`] — versioned [`AgentCommand`] / [`AgentResponse`] enums
//!   serialized as JSON over plain HTTP (`/v1/...`).
//! * [`queue`] — a thread-safe MPSC queue plus per-command reply
//!   channels. The HTTP worker thread enqueues a command and blocks on
//!   the reply; the game thread drains the queue once per frame.
//! * [`log_sink`] — bounded ring-buffer [`log::Log`] adapter so
//!   `/v1/log/tail` can return recent records cheaply.
//! * [`session`] — the [`AgentSession`] trait each per-game adapter
//!   implements (PAL4 first; PAL3 / PAL5 in follow-ups).
//! * [`transport`] — the embedded [`AgentServer`] that wires the
//!   three together.
//!
//! No async runtime is pulled in: the listener uses a dedicated
//! `tiny_http` thread plus standard library channels.

pub mod log_sink;
pub mod protocol;
pub mod queue;
pub mod session;
pub mod transport;

pub use log_sink::{AgentLogSink, LogRecord};
pub use protocol::{
    AgentCommand, AgentError, AgentErrorKind, AgentResponse, DialogSnapshot, KeyAction,
    PartyMember, StateSnapshot,
};
pub use queue::{AgentCommandConsumer, AgentCommandQueue, AgentEnvelope};
pub use session::{AgentSession, NullAgentSession};
pub use transport::{AgentServer, AgentServerConfig};
