//! End-to-end HTTP transport test.
//!
//! Spins up a real `AgentServer` bound to an ephemeral loopback port,
//! drives commands through a worker thread that wraps a
//! [`NullAgentSession`], and asserts:
//!
//! * the listener accepts requests and routes them to the queue,
//! * the worker's response flows back to the HTTP client,
//! * `/v1/log/tail` is served directly from the sink without going
//!   through the queue,
//! * unknown routes return a structured `Error` payload.
//!
//! No real game state is involved — this is a transport smoke test
//! whose value is catching regressions in the queue / dispatch / JSON
//! wiring before they reach a real session.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use agent_server::{
    AgentCommandQueue, AgentLogSink, AgentResponse, AgentServer, AgentServerConfig, AgentSession,
    NullAgentSession,
};

/// Spawn a worker thread that drains the queue and dispatches into a
/// `NullAgentSession`. Returns a handle that, when dropped, signals
/// shutdown and joins the thread.
struct WorkerHandle {
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn spawn_worker(consumer: agent_server::AgentCommandConsumer) -> WorkerHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_w = stop.clone();
    let join = thread::Builder::new()
        .name("agent-test-worker".to_string())
        .spawn(move || {
            let mut session = NullAgentSession::new();
            while !stop_w.load(Ordering::SeqCst) {
                let _ = consumer.drain_with_timeout(Duration::from_millis(50), |env| {
                    let cmd = env.command.clone();
                    let resp = session.execute(cmd);
                    env.reply(resp);
                });
            }
        })
        .expect("spawn worker");
    WorkerHandle {
        stop,
        join: Some(join),
    }
}

/// Issue a one-shot HTTP request and return (status, body).
fn http_request(
    addr: &std::net::SocketAddr,
    method: &str,
    path: &str,
    body: &str,
) -> (u16, String) {
    let (status, _headers, body) = http_request_raw(addr, method, path, body);
    (status, String::from_utf8_lossy(&body).into_owned())
}

/// Same as [`http_request`] but returns raw response bytes plus the
/// parsed header lines so binary endpoints (e.g. `/v1/screenshot`)
/// can be exercised without losing the payload to UTF-8 conversion.
fn http_request_raw(
    addr: &std::net::SocketAddr,
    method: &str,
    path: &str,
    body: &str,
) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let mut stream =
        TcpStream::connect_timeout(addr, Duration::from_secs(2)).expect("connect to agent server");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
        host = addr,
        len = body.len(),
    );
    stream.write_all(req.as_bytes()).unwrap();
    stream.flush().unwrap();

    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();

    // Split head / body on the first CRLFCRLF sequence so the body
    // stays byte-faithful (the body may contain non-UTF-8 bytes).
    let sep = b"\r\n\r\n";
    let split_at = response
        .windows(sep.len())
        .position(|w| w == sep)
        .map(|p| p + sep.len())
        .unwrap_or(response.len());
    let (head, body) = response.split_at(split_at);
    let head = String::from_utf8_lossy(&head[..head.len().saturating_sub(sep.len())]);

    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or("");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0u16);

    let headers: Vec<(String, String)> = lines
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            Some((k.trim().to_ascii_lowercase(), v.trim().to_string()))
        })
        .collect();

    (status, headers, body.to_vec())
}

fn wait_for_server(addr: &std::net::SocketAddr) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if TcpStream::connect_timeout(addr, Duration::from_millis(100)).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("agent server didn't accept connections within 2 s");
}

#[test]
fn server_round_trips_get_state_through_worker() {
    let (queue, consumer) = AgentCommandQueue::new();
    let server = AgentServer::start(AgentServerConfig::loopback(0), &queue, None)
        .expect("start agent server");
    let addr = server.local_addr();
    let _worker = spawn_worker(consumer);
    wait_for_server(&addr);

    let (status, body) = http_request(&addr, "GET", "/v1/state", "");
    assert_eq!(status, 200, "body={body}");

    let resp: AgentResponse = serde_json::from_str(&body).expect(&format!("parse: {body}"));
    match resp {
        AgentResponse::State(s) => {
            // NullAgentSession bumps frame on every execute.
            assert!(s.frame >= 1, "frame should advance: {s:?}");
        }
        other => panic!("expected State, got {other:?}"),
    }
}

#[test]
fn server_round_trips_post_input_key() {
    let (queue, consumer) = AgentCommandQueue::new();
    let server = AgentServer::start(AgentServerConfig::loopback(0), &queue, None)
        .expect("start agent server");
    let addr = server.local_addr();
    let _worker = spawn_worker(consumer);
    wait_for_server(&addr);

    let body = r#"{"key":"F","action":"tap"}"#;
    let (status, resp_body) = http_request(&addr, "POST", "/v1/input/key", body);
    assert_eq!(status, 200, "body={resp_body}");
    let resp: AgentResponse =
        serde_json::from_str(&resp_body).expect(&format!("parse: {resp_body}"));
    assert!(matches!(resp, AgentResponse::Ok), "got {resp:?}");
}

#[test]
fn log_tail_is_served_without_worker() {
    // Install a sink, push a record, then start the server WITHOUT a
    // worker — proves `/v1/log/tail` doesn't route through the queue.
    static SINK: std::sync::OnceLock<&'static AgentLogSink> = std::sync::OnceLock::new();
    let sink = *SINK.get_or_init(|| {
        // Don't install as global logger (that'd race with other
        // tests); use the `record_external` side channel instead.
        Box::leak(Box::new(AgentLogSink::new(64)))
    });
    sink.record_external(log::Level::Info, "test", "hello world");

    let (queue, _consumer) = AgentCommandQueue::new();
    let server = AgentServer::start(AgentServerConfig::loopback(0), &queue, Some(sink))
        .expect("start agent server");
    let addr = server.local_addr();
    wait_for_server(&addr);

    let (status, body) = http_request(&addr, "GET", "/v1/log/tail?after_seq=0&n=10", "");
    assert_eq!(status, 200, "body={body}");
    let resp: AgentResponse = serde_json::from_str(&body).expect(&format!("parse: {body}"));
    match resp {
        AgentResponse::Log(l) => {
            assert!(!l.records.is_empty(), "expected at least one log record");
            assert!(l.records.iter().any(|r| r.msg == "hello world"));
            assert!(l.next_seq >= 1);
        }
        other => panic!("expected Log, got {other:?}"),
    }
}

#[test]
fn unknown_route_returns_error() {
    let (queue, _consumer) = AgentCommandQueue::new();
    let server = AgentServer::start(AgentServerConfig::loopback(0), &queue, None)
        .expect("start agent server");
    let addr = server.local_addr();
    wait_for_server(&addr);

    let (status, body) = http_request(&addr, "GET", "/v1/nope", "");
    assert_eq!(status, 400, "body={body}");
    let resp: AgentResponse = serde_json::from_str(&body).expect(&format!("parse: {body}"));
    assert!(matches!(resp, AgentResponse::Error(_)), "got {resp:?}");
}

#[test]
fn token_protected_server_rejects_anonymous_requests() {
    let (queue, consumer) = AgentCommandQueue::new();
    let server = AgentServer::start(
        AgentServerConfig::loopback(0).with_token("hunter2"),
        &queue,
        None,
    )
    .expect("start agent server");
    let addr = server.local_addr();
    let _worker = spawn_worker(consumer);
    wait_for_server(&addr);

    let (status, body) = http_request(&addr, "GET", "/v1/state", "");
    assert_eq!(status, 401, "body={body}");
}

/// Spawn a worker that returns a fixed 2×2 RGBA frame from
/// `/v1/screenshot`. Lets the e2e test below exercise the binary
/// response path without booting a real Vulkan engine.
fn spawn_screenshot_worker(
    consumer: agent_server::AgentCommandConsumer,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
) -> WorkerHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_w = stop.clone();
    let join = thread::Builder::new()
        .name("agent-screenshot-worker".to_string())
        .spawn(move || {
            let mut session = NullAgentSession::new();
            session.set_screenshot(width, height, rgba);
            while !stop_w.load(Ordering::SeqCst) {
                let _ = consumer.drain_with_timeout(Duration::from_millis(50), |env| {
                    let cmd = env.command.clone();
                    let resp = session.execute(cmd);
                    env.reply(resp);
                });
            }
        })
        .expect("spawn screenshot worker");
    WorkerHandle {
        stop,
        join: Some(join),
    }
}

#[test]
fn screenshot_endpoint_returns_binary_png() {
    // 2×2 RGBA: red, green, blue, white.
    let rgba: Vec<u8> = vec![
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    let (queue, consumer) = AgentCommandQueue::new();
    let server = AgentServer::start(AgentServerConfig::loopback(0), &queue, None)
        .expect("start agent server");
    let addr = server.local_addr();
    let _worker = spawn_screenshot_worker(consumer, 2, 2, rgba.clone());
    wait_for_server(&addr);

    let (status, headers, body) = http_request_raw(&addr, "GET", "/v1/screenshot", "");
    assert_eq!(status, 200, "headers={headers:?}");

    let content_type = headers
        .iter()
        .find(|(k, _)| k == "content-type")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    assert_eq!(content_type, "image/png", "headers={headers:?}");

    let width_hdr = headers
        .iter()
        .find(|(k, _)| k == "x-screenshot-width")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    let height_hdr = headers
        .iter()
        .find(|(k, _)| k == "x-screenshot-height")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    assert_eq!(width_hdr, "2");
    assert_eq!(height_hdr, "2");

    // Decode the PNG and verify dimensions + first pixel.
    let img = image::load_from_memory_with_format(&body, image::ImageFormat::Png)
        .expect("response body must be a valid PNG");
    let rgba_img = img.to_rgba8();
    assert_eq!(rgba_img.dimensions(), (2, 2));
    let p = rgba_img.get_pixel(0, 0);
    assert_eq!(p.0, [255, 0, 0, 255], "first pixel should be opaque red");
}

#[test]
fn screenshot_endpoint_returns_501_when_capture_unavailable() {
    let (queue, consumer) = AgentCommandQueue::new();
    let server = AgentServer::start(AgentServerConfig::loopback(0), &queue, None)
        .expect("start agent server");
    let addr = server.local_addr();
    // Default NullAgentSession returns an empty ScreenshotResponse;
    // transport must convert that into a 501.
    let _worker = spawn_worker(consumer);
    wait_for_server(&addr);

    let (status, body) = http_request(&addr, "GET", "/v1/screenshot", "");
    assert_eq!(status, 501, "body={body}");
    let resp: AgentResponse = serde_json::from_str(&body).expect(&format!("parse: {body}"));
    assert!(matches!(resp, AgentResponse::Error(_)), "got {resp:?}");
}
