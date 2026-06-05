//! PAL4 agent-server boot helpers (Rust-side).
//!
//! Phase 1 named this `launch.rs` and included a `Pal4LaunchContext`
//! ComObject. Phase 2 moved director construction into `Pal4Service`
//! (which lives next to this module). This file now contains only the
//! agent-server lifecycle helpers used by `YaobowApplicationLoader`:
//!
//!  * [`AgentBootOptions`] — port / bind / token / reply-timeout
//!    parsed from `--pal4 --agent-port` CLI args.
//!  * [`install_global_log_sink`] — installs (or reuses) the global
//!    `AgentLogSink` so `/v1/log/tail` works.
//!  * [`start_agent_server`] — boots the HTTP listener thread against
//!    the supplied [`Pal4AgentBridge`].
//!
//! The owner of the resulting [`AgentServer`] handle is
//! `YaobowApplicationLoader` (not `Pal4Service` — see rubber-duck
//! finding A in `plan_v2.md`): keeping it there guarantees the
//! listener thread is joined at process exit and not tied to scripted
//! ownership cycles.

use std::rc::Rc;
use std::sync::OnceLock;

use agent_server::{AgentLogSink, AgentServer, AgentServerConfig};

use crate::openpal4::agent::Pal4AgentBridge;

/// Boot-time options for the embedded agent server. `None` keeps the
/// classic windowed-only flow with no extra threads or input wrapping.
#[derive(Debug, Clone)]
pub struct AgentBootOptions {
    /// Port to bind. `0` selects an ephemeral port (mostly useful for
    /// tests that need to discover the address via [`AgentServer::local_addr`]).
    pub port: u16,
    /// Optional bind override. Defaults to `127.0.0.1` (loopback).
    pub bind_ip: Option<std::net::IpAddr>,
    /// Bearer token. Required for non-loopback binds.
    pub token: Option<String>,
    /// Per-request HTTP reply timeout. The transport returns HTTP 500
    /// with `"game thread did not reply within {dur:?}"` if the
    /// game-side handler doesn't reply in time. `None` keeps the
    /// crate default (5 s).
    pub reply_timeout: Option<std::time::Duration>,
}

impl AgentBootOptions {
    pub fn loopback(port: u16) -> Self {
        Self {
            port,
            bind_ip: None,
            token: None,
            reply_timeout: None,
        }
    }
}

/// Lazy-install a single global `AgentLogSink` so multiple boots in
/// the same process (e.g. integration tests) share the same ring
/// buffer instead of fighting over `log::set_logger`.
pub fn install_global_log_sink() -> Option<&'static AgentLogSink> {
    static SINK: OnceLock<&'static AgentLogSink> = OnceLock::new();
    if let Some(s) = SINK.get() {
        return Some(*s);
    }

    if let Some(existing) = AgentLogSink::global() {
        let _ = SINK.set(existing);
        return Some(existing);
    }

    match AgentLogSink::new(4096).install() {
        Ok(s) => {
            let _ = SINK.set(s);
            Some(s)
        }
        Err(_) => {
            log::warn!("agent_server: global logger already installed; /v1/log/tail will be empty");
            None
        }
    }
}

/// Boot the embedded agent-server HTTP listener thread against the
/// supplied [`Pal4AgentBridge`]. The returned [`AgentServer`] handle
/// owns the listener; dropping it joins the thread.
pub fn start_agent_server(
    opts: &AgentBootOptions,
    bridge: &Rc<Pal4AgentBridge>,
    log_sink: Option<&'static AgentLogSink>,
) -> Result<AgentServer, String> {
    let mut config = AgentServerConfig::loopback(opts.port);
    if let Some(ip) = opts.bind_ip {
        config = config.with_bind(std::net::SocketAddr::new(ip, opts.port))?;
    }
    if let Some(token) = opts.token.clone() {
        config = config.with_token(token);
    }
    if let Some(reply_timeout) = opts.reply_timeout {
        config = config.with_reply_timeout(reply_timeout);
    }
    AgentServer::start(config, &bridge.queue, log_sink)
}
