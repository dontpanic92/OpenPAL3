//! Embedded HTTP+JSON transport.
//!
//! [`AgentServer::start`] spins a single listener thread that owns a
//! `tiny_http::Server`. For every incoming request the thread:
//!
//! 1. Authenticates (optional bearer token).
//! 2. Routes the URL+method to an [`AgentCommand`] constructor.
//! 3. Pushes an [`AgentEnvelope`](crate::AgentEnvelope) onto the
//!    [`AgentCommandQueue`](crate::AgentCommandQueue) shared with the
//!    game thread.
//! 4. Blocks on the envelope's reply channel (with a timeout) and
//!    streams the resulting [`AgentResponse`](crate::AgentResponse)
//!    back to the client.
//!
//! Drop the returned [`AgentServer`] handle to stop the listener and
//! join the thread.

// `parse_post_command` reads the body via `as_reader().read_to_string`,
// which works without the `Read` trait in scope because `tiny_http`
// exposes the body reader as a concrete `&mut dyn Read` whose inherent
// methods cover what we need.
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::Deserialize;
use tiny_http::{Header, Method, Request, Response, Server};

use crate::log_sink::{AgentLogSink, LogRecord};
use crate::protocol::{
    AgentCommand, AgentError, AgentErrorKind, AgentResponse, AxisInputParams, FastForwardParams,
    KeyInputParams, LogRecordPayload, LogTailParams, LogTailResponse, NameParams,
    ScreenshotResponse, ScriptEvalParams, ScriptGlobalsParams, SlotParams, StepTimeParams,
    TeleportParams,
};
use crate::queue::{AgentCommandQueue, AgentEnvelope};

/// Listener configuration.
#[derive(Debug, Clone)]
pub struct AgentServerConfig {
    /// Address to bind. Defaults to `127.0.0.1:0`.
    pub bind: SocketAddr,
    /// If `Some`, every request must carry
    /// `Authorization: Bearer <token>`. Required when [`Self::bind`]
    /// is non-loopback.
    pub token: Option<String>,
    /// How long the HTTP worker waits for the game thread to reply
    /// before returning HTTP 504. Defaults to 5 seconds.
    pub reply_timeout: Duration,
}

impl Default for AgentServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:0".parse().expect("default loopback addr parses"),
            token: None,
            reply_timeout: Duration::from_secs(5),
        }
    }
}

impl AgentServerConfig {
    pub fn loopback(port: u16) -> Self {
        Self {
            bind: SocketAddr::from(([127, 0, 0, 1], port)),
            ..Self::default()
        }
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    pub fn with_bind(mut self, bind: impl ToSocketAddrs) -> Result<Self, String> {
        let addr = bind
            .to_socket_addrs()
            .map_err(|e| format!("resolve bind: {e}"))?
            .next()
            .ok_or_else(|| "bind address resolved to nothing".to_string())?;
        self.bind = addr;
        Ok(self)
    }

    fn requires_token(&self) -> bool {
        !self.bind.ip().is_loopback() && self.token.is_none()
    }
}

/// Owning handle to the listener thread. Drop to stop.
pub struct AgentServer {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl AgentServer {
    /// Spawn the listener thread.
    ///
    /// `queue.sender()` is cloned into the worker so multiple servers
    /// (e.g. one per game) can share a single drain loop if desired.
    /// `log_sink` is optional; when `None`, `/v1/log/tail` returns an
    /// empty page.
    pub fn start(
        config: AgentServerConfig,
        queue: &AgentCommandQueue,
        log_sink: Option<&'static AgentLogSink>,
    ) -> Result<Self, String> {
        if config.requires_token() {
            return Err(
                "non-loopback binds require a token; set AgentServerConfig::token".to_string(),
            );
        }

        let server = Server::http(config.bind).map_err(|e| format!("bind {}: {e}", config.bind))?;
        let addr = server
            .server_addr()
            .to_ip()
            .ok_or_else(|| "tiny_http reported a non-IP server address".to_string())?;
        let shutdown = Arc::new(AtomicBool::new(false));

        let sender = queue.sender();
        let token = config.token.clone();
        let timeout = config.reply_timeout;
        let shutdown_worker = shutdown.clone();

        let join = thread::Builder::new()
            .name("agent-server".to_string())
            .spawn(move || {
                listener_loop(server, sender, token, timeout, log_sink, shutdown_worker);
            })
            .map_err(|e| format!("spawn agent-server thread: {e}"))?;

        Ok(Self {
            addr,
            shutdown,
            join: Some(join),
        })
    }

    /// Bound address (useful when `port == 0`).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for AgentServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Kick the listener out of its blocking `recv_timeout` by
        // dialing it once. Errors are ignored — if the thread already
        // exited there's nothing to do.
        let _ = std::net::TcpStream::connect_timeout(&self.addr, Duration::from_millis(100));
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

fn listener_loop(
    server: Server,
    sender: Sender<AgentEnvelope>,
    token: Option<String>,
    timeout: Duration,
    log_sink: Option<&'static AgentLogSink>,
    shutdown: Arc<AtomicBool>,
) {
    // `recv_timeout` lets the loop notice the shutdown flag without
    // depending on `tiny_http`'s lack of `try_recv`.
    loop {
        if shutdown.load(Ordering::SeqCst) {
            return;
        }
        let req = match server.recv_timeout(Duration::from_millis(200)) {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(err) => {
                log::error!("agent_server: listener error: {err}");
                continue;
            }
        };

        if shutdown.load(Ordering::SeqCst) {
            // Reply with a quick 503 so the shutdown probe doesn't
            // hang on the client side.
            let _ = req.respond(empty_response(503));
            continue;
        }

        handle_request(req, &sender, token.as_deref(), timeout, log_sink);
    }
}

fn handle_request(
    mut req: Request,
    sender: &Sender<AgentEnvelope>,
    token: Option<&str>,
    timeout: Duration,
    log_sink: Option<&'static AgentLogSink>,
) {
    if let Err(err) = check_auth(&req, token) {
        let _ = respond_error(req, AgentError::unauthorized(err));
        return;
    }

    let url = req.url().to_string();
    let method = req.method().clone();

    // `/v1/log/tail` is served directly from the sink so log polling
    // doesn't have to round-trip through the game thread.
    if method == Method::Get && url.starts_with("/v1/log/tail") {
        let params = match parse_log_tail_query(&url) {
            Ok(p) => p,
            Err(e) => {
                let _ = respond_error(req, AgentError::bad_request(e));
                return;
            }
        };
        let response = build_log_tail_response(params, log_sink);
        let _ = respond_json(req, 200, &response);
        return;
    }

    // Convenience endpoints: GET /v1/state, GET /v1/screenshot,
    // GET /v1/scene/{triggers,objects}, GET /v1/script/globals.
    let command = match (method.clone(), url.as_str()) {
        (Method::Get, "/v1/state") => Ok(AgentCommand::GetState),
        (Method::Get, "/v1/screenshot") => Ok(AgentCommand::Screenshot),
        (Method::Get, "/v1/scene/triggers") => Ok(AgentCommand::GetSceneTriggers),
        (Method::Get, "/v1/scene/objects") => Ok(AgentCommand::GetSceneObjects),
        (Method::Get, url_str) if url_str.starts_with("/v1/script/globals") => {
            parse_script_globals_query(url_str)
                .map(AgentCommand::GetScriptGlobals)
                .map_err(AgentError::bad_request)
        }
        (Method::Post, _) => parse_post_command(&url, &mut req),
        _ => Err(AgentError::bad_request(format!(
            "unsupported route: {method} {url}"
        ))),
    };

    let command = match command {
        Ok(c) => c,
        Err(e) => {
            let _ = respond_error(req, e);
            return;
        }
    };

    let (env, rx) = AgentEnvelope::new(command);
    if let Err(err) = sender.send(env) {
        let _ = respond_error(req, AgentError::internal(format!("queue closed: {err}")));
        return;
    }

    match rx.recv_timeout(timeout) {
        Ok(response) => {
            let _ = respond_dispatched(req, response);
        }
        Err(RecvTimeoutError::Timeout) => {
            let _ = respond_error(
                req,
                AgentError {
                    kind: AgentErrorKind::Internal,
                    message: format!("game thread did not reply within {:?}", timeout),
                },
            );
        }
        Err(RecvTimeoutError::Disconnected) => {
            let _ = respond_error(
                req,
                AgentError::internal("game thread dropped the reply channel"),
            );
        }
    }
}

/// Branch on the response variant to choose between JSON and binary
/// encoding. Screenshot responses skip JSON and emit `image/png`
/// directly; everything else flows through `respond_json`.
fn respond_dispatched(req: Request, response: AgentResponse) -> std::io::Result<()> {
    match response {
        AgentResponse::Screenshot(payload) => respond_screenshot(req, payload),
        other => respond_json(req, status_for(&other), &other),
    }
}

fn respond_screenshot(req: Request, payload: ScreenshotResponse) -> std::io::Result<()> {
    // Empty captures (no swapchain, headless, unsupported format)
    // produce a structured 501 so curl users get a useful message
    // instead of an empty PNG.
    if payload.rgba.is_empty() || payload.width == 0 || payload.height == 0 {
        return respond_error(
            req,
            AgentError::not_implemented(
                "screenshot unavailable (no presented frame or unsupported swapchain format)",
            ),
        );
    }

    let expected_len = (payload.width as usize) * (payload.height as usize) * 4;
    if payload.rgba.len() != expected_len {
        return respond_error(
            req,
            AgentError::internal(format!(
                "screenshot payload length {} doesn't match width*height*4 = {}",
                payload.rgba.len(),
                expected_len
            )),
        );
    }

    let mut png_bytes: Vec<u8> = Vec::new();
    let encode_result = {
        let buffer = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
            payload.width,
            payload.height,
            payload.rgba,
        );
        match buffer {
            Some(buf) => image::DynamicImage::ImageRgba8(buf)
                .write_to(&mut png_bytes, image::ImageOutputFormat::Png),
            None => Err(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            )),
        }
    };

    if let Err(e) = encode_result {
        return respond_error(req, AgentError::internal(format!("png encode failed: {e}")));
    }

    let content_type = Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..])
        .expect("static content-type parses");
    let width_header_value = format!("{}", payload.width);
    let height_header_value = format!("{}", payload.height);
    let width_header =
        Header::from_bytes(&b"X-Screenshot-Width"[..], width_header_value.as_bytes())
            .expect("width header parses");
    let height_header =
        Header::from_bytes(&b"X-Screenshot-Height"[..], height_header_value.as_bytes())
            .expect("height header parses");

    let response = Response::from_data(png_bytes)
        .with_status_code(200)
        .with_header(content_type)
        .with_header(width_header)
        .with_header(height_header);
    req.respond(response)
}

fn parse_post_command(url: &str, req: &mut Request) -> Result<AgentCommand, AgentError> {
    let mut body = String::new();
    req.as_reader()
        .read_to_string(&mut body)
        .map_err(|e| AgentError::bad_request(format!("read body: {e}")))?;

    fn parse<'a, T: Deserialize<'a>>(body: &'a str) -> Result<T, AgentError> {
        if body.trim().is_empty() {
            // Synthesize a default-shaped JSON so endpoints with no
            // params can be POSTed with an empty body.
            serde_json::from_str("{}")
                .map_err(|e| AgentError::bad_request(format!("empty body, default failed: {e}")))
        } else {
            serde_json::from_str(body)
                .map_err(|e| AgentError::bad_request(format!("parse body: {e}")))
        }
    }

    let cmd = match url {
        "/v1/input/key" => AgentCommand::KeyInput(parse::<KeyInputParams>(&body)?),
        "/v1/input/axis" => AgentCommand::AxisInput(parse::<AxisInputParams>(&body)?),
        "/v1/player/teleport" => AgentCommand::TeleportPlayer(parse::<TeleportParams>(&body)?),
        "/v1/dialog/advance" => AgentCommand::AdvanceDialog,
        "/v1/time/pause" => AgentCommand::PauseTime,
        "/v1/time/resume" => AgentCommand::ResumeTime,
        "/v1/time/step" => AgentCommand::StepTime(parse::<StepTimeParams>(&body)?),
        "/v1/time/fast_forward" => AgentCommand::FastForward(parse::<FastForwardParams>(&body)?),
        "/v1/save" => AgentCommand::SaveSlot(parse::<SlotParams>(&body)?),
        "/v1/load" => AgentCommand::LoadSlot(parse::<SlotParams>(&body)?),
        "/v1/script/eval" => AgentCommand::ScriptEval(parse::<ScriptEvalParams>(&body)?),
        "/v1/scene/fire_trigger" => AgentCommand::FireSceneTrigger(parse::<NameParams>(&body)?),
        "/v1/object/interact" => AgentCommand::InteractObject(parse::<NameParams>(&body)?),
        _ => {
            return Err(AgentError::bad_request(format!(
                "unknown POST route: {url}"
            )))
        }
    };

    Ok(cmd)
}

fn check_auth(req: &Request, token: Option<&str>) -> Result<(), String> {
    let Some(token) = token else { return Ok(()) };
    let expected = format!("Bearer {token}");
    let ok = req.headers().iter().any(|h| {
        h.field
            .as_str()
            .as_str()
            .eq_ignore_ascii_case("authorization")
            && h.value.as_str() == expected
    });
    if ok {
        Ok(())
    } else {
        Err("missing or invalid Authorization header".to_string())
    }
}

fn parse_log_tail_query(url: &str) -> Result<LogTailParams, String> {
    let q = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut after_seq: u64 = 0;
    let mut n: Option<usize> = None;
    for pair in q.split('&').filter(|s| !s.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "after_seq" => {
                after_seq = v.parse().map_err(|e| format!("after_seq: {e}"))?;
            }
            "n" => {
                n = Some(v.parse().map_err(|e| format!("n: {e}"))?);
            }
            _ => {}
        }
    }
    Ok(LogTailParams { after_seq, n })
}

fn parse_script_globals_query(url: &str) -> Result<ScriptGlobalsParams, String> {
    let q = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut start: usize = 0;
    let mut limit: Option<usize> = None;
    for pair in q.split('&').filter(|s| !s.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "start" => {
                start = v.parse().map_err(|e| format!("start: {e}"))?;
            }
            "limit" => {
                limit = Some(v.parse().map_err(|e| format!("limit: {e}"))?);
            }
            _ => {}
        }
    }
    Ok(ScriptGlobalsParams { start, limit })
}

fn build_log_tail_response(
    params: LogTailParams,
    log_sink: Option<&'static AgentLogSink>,
) -> AgentResponse {
    let n = params.n.unwrap_or(256).min(2048);
    let (records, next_seq, dropped) = match log_sink {
        Some(sink) => sink.tail(params.after_seq, n),
        None => (Vec::<LogRecord>::new(), params.after_seq, false),
    };
    AgentResponse::Log(LogTailResponse {
        next_seq,
        dropped,
        records: records.into_iter().map(record_to_payload).collect(),
    })
}

fn record_to_payload(rec: LogRecord) -> LogRecordPayload {
    LogRecordPayload {
        seq: rec.seq,
        ts: None,
        level: level_to_string(rec.level).to_string(),
        target: rec.target,
        msg: rec.message,
    }
}

fn level_to_string(level: log::Level) -> &'static str {
    match level {
        log::Level::Error => "error",
        log::Level::Warn => "warn",
        log::Level::Info => "info",
        log::Level::Debug => "debug",
        log::Level::Trace => "trace",
    }
}

fn status_for(response: &AgentResponse) -> u16 {
    match response {
        AgentResponse::Error(e) => match e.kind {
            AgentErrorKind::BadRequest => 400,
            AgentErrorKind::Unauthorized => 401,
            AgentErrorKind::Conflict => 409,
            AgentErrorKind::NotImplemented => 501,
            AgentErrorKind::Internal => 500,
        },
        _ => 200,
    }
}

fn respond_error(req: Request, err: AgentError) -> std::io::Result<()> {
    let status = status_for(&AgentResponse::Error(err.clone()));
    respond_json(req, status, &AgentResponse::Error(err))
}

fn respond_json<T: serde::Serialize>(
    req: Request,
    status: u16,
    payload: &T,
) -> std::io::Result<()> {
    let body = match serde_json::to_vec(payload) {
        Ok(b) => b,
        Err(e) => format!("{{\"error\":\"serialize: {e}\"}}").into_bytes(),
    };
    let header = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
        .expect("static content-type header parses");
    let response = Response::from_data(body)
        .with_status_code(status)
        .with_header(header);
    req.respond(response)
}

fn empty_response(status: u16) -> Response<std::io::Empty> {
    Response::empty(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_log_tail_query_basic() {
        let p = parse_log_tail_query("/v1/log/tail?after_seq=12&n=50").unwrap();
        assert_eq!(p.after_seq, 12);
        assert_eq!(p.n, Some(50));
    }

    #[test]
    fn parse_log_tail_query_defaults() {
        let p = parse_log_tail_query("/v1/log/tail").unwrap();
        assert_eq!(p.after_seq, 0);
        assert_eq!(p.n, None);
    }

    #[test]
    fn parse_script_globals_query_basic() {
        let p = parse_script_globals_query("/v1/script/globals?start=8&limit=32").unwrap();
        assert_eq!(p.start, 8);
        assert_eq!(p.limit, Some(32));
    }

    #[test]
    fn parse_script_globals_query_defaults() {
        let p = parse_script_globals_query("/v1/script/globals").unwrap();
        assert_eq!(p.start, 0);
        assert_eq!(p.limit, None);
    }
}
