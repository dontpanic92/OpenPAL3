//! Game-agnostic agent-server boot helpers.
//!
//! These were originally `shared::openpal4::launch` because PAL4 was
//! the first game to grow an agent surface. Hoisting them into
//! `agent_common` lets PAL3 (and future games) reuse the same option
//! parsing and listener wiring.
//!
//! `openpal4::launch` keeps a thin re-export for backward compatibility.

use std::rc::Rc;
use std::sync::OnceLock;

use agent_server::{AgentLogSink, AgentServer, AgentServerConfig};

use crate::agent_common::bridge::AgentBridge;

/// Boot-time options for the embedded agent server. `None` keeps the
/// classic windowed-only flow with no extra threads or input wrapping.
#[derive(Debug, Clone)]
pub struct AgentBootOptions {
    /// Port to bind. `0` selects an ephemeral port (mostly useful for
    /// tests that need to discover the address via
    /// [`AgentServer::local_addr`]).
    pub port: u16,
    /// Optional bind override. Defaults to `127.0.0.1` (loopback).
    pub bind_ip: Option<std::net::IpAddr>,
    /// Bearer token. Required for non-loopback binds.
    pub token: Option<String>,
    /// Per-request HTTP reply timeout. The transport returns HTTP
    /// 500 with `"game thread did not reply within {dur:?}"` if the
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
/// supplied [`AgentBridge`]. The returned [`AgentServer`] handle
/// owns the listener; dropping it joins the thread.
pub fn start_agent_server(
    opts: &AgentBootOptions,
    bridge: &Rc<AgentBridge>,
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
