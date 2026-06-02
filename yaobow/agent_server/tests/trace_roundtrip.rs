//! End-to-end test for the trace endpoints.
//!
//! Spins up a real `AgentServer` plus a worker that owns an
//! `AgentTraceSink`. The worker translates `TraceStart` / `TraceStop`
//! / `TraceDrain` into sink calls so the test can:
//!
//! 1. Start tracing.
//! 2. Inject synthetic events into the sink (simulating VM activity).
//! 3. Drain via HTTP and assert the wire payload shape.
//! 4. Verify the cursor semantics and `dropped` flag.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use agent_server::protocol::{
    AgentCommand, AgentResponse, TraceDrainResponse, TraceEventKindPayload, TraceEventPayload,
};
use agent_server::{
    AgentCommandConsumer, AgentCommandQueue, AgentServer, AgentServerConfig, AgentTraceSink,
};

/// Worker that translates trace commands into the shared sink.
struct TraceWorker {
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

impl Drop for TraceWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn spawn_trace_worker(
    consumer: AgentCommandConsumer,
    sink: Arc<AgentTraceSink>,
) -> TraceWorker {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_w = stop.clone();
    let join = thread::Builder::new()
        .name("trace-test-worker".to_string())
        .spawn(move || {
            while !stop_w.load(Ordering::SeqCst) {
                let _ = consumer.drain_with_timeout(Duration::from_millis(20), |env| {
                    let resp = match env.command.clone() {
                        AgentCommand::TraceStart(p) => {
                            sink.start(p.reset, p.capacity);
                            AgentResponse::Ok
                        }
                        AgentCommand::TraceStop => {
                            sink.stop();
                            AgentResponse::Ok
                        }
                        AgentCommand::TraceDrain(p) => {
                            let n = p.n.unwrap_or(1024);
                            let r = sink.drain(p.after_seq, n);
                            AgentResponse::TraceDrain(TraceDrainResponse {
                                next_seq: r.next_seq,
                                dropped: r.dropped,
                                capturing: sink.is_capturing(),
                                events: r.events,
                            })
                        }
                        _ => AgentResponse::Ok,
                    };
                    env.reply(resp);
                });
            }
        })
        .expect("spawn worker");
    TraceWorker {
        stop,
        join: Some(join),
    }
}

fn http_request(addr: &SocketAddr, method: &str, path: &str, body: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
        method = method,
        path = path,
        addr = addr,
        len = body.len(),
        body = body,
    );
    s.write_all(req.as_bytes()).expect("write");
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).expect("read");
    let text = String::from_utf8_lossy(&buf).into_owned();
    let status: u16 = text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1)?.parse().ok())
        .unwrap_or(0);
    let body_start = text.find("\r\n\r\n").map(|i| i + 4).unwrap_or(text.len());
    (status, text[body_start..].to_string())
}

fn parse_drain(body: &str) -> TraceDrainResponse {
    #[derive(serde::Deserialize)]
    struct Env {
        #[serde(rename = "type")]
        ty: String,
        data: TraceDrainResponse,
    }
    let env: Env = serde_json::from_str(body).expect("parse drain body");
    assert_eq!(env.ty, "trace_drain", "wire tag mismatch in: {body}");
    env.data
}

fn suspend(name: &str) -> TraceEventPayload {
    TraceEventPayload {
        seq: 0,
        kind: TraceEventKindPayload::Suspend {
            fn_name: name.to_string(),
            pc: 0,
        },
    }
}

#[test]
fn trace_drain_round_trips_pushed_events() {
    let (queue, consumer) = AgentCommandQueue::new();
    let sink = Arc::new(AgentTraceSink::with_default_capacity());
    let _worker = spawn_trace_worker(consumer, sink.clone());
    let server = AgentServer::start(AgentServerConfig::loopback(0), &queue, None)
        .expect("start agent server");
    let addr = server.local_addr();

    // 1. Start tracing.
    let (status, body) = http_request(&addr, "POST", "/v1/script/trace/start", "{}");
    assert_eq!(status, 200, "trace/start body: {body}");

    // 2. Push 3 synthetic events into the sink as if the VM had
    //    emitted them.
    for name in &["a", "b", "c"] {
        sink.push(suspend(name));
    }

    // 3. Drain via HTTP.
    let (status, body) = http_request(&addr, "GET", "/v1/script/trace/drain?after_seq=0", "");
    assert_eq!(status, 200, "drain body: {body}");
    let drain = parse_drain(&body);
    assert!(drain.capturing);
    assert!(!drain.dropped);
    assert_eq!(drain.next_seq, 4);
    let names: Vec<String> = drain
        .events
        .iter()
        .map(|e| match &e.kind {
            TraceEventKindPayload::Suspend { fn_name, .. } => fn_name.clone(),
            other => panic!("unexpected kind: {:?}", other),
        })
        .collect();
    assert_eq!(names, vec!["a", "b", "c"]);

    // 4. Drain incremental — push one more, ask for events past last seq.
    sink.push(suspend("d"));
    let (status, body) = http_request(&addr, "GET", "/v1/script/trace/drain?after_seq=3", "");
    assert_eq!(status, 200, "drain2 body: {body}");
    let drain = parse_drain(&body);
    assert_eq!(drain.next_seq, 5);
    assert_eq!(drain.events.len(), 1);
    match &drain.events[0].kind {
        TraceEventKindPayload::Suspend { fn_name, .. } => assert_eq!(fn_name, "d"),
        other => panic!("unexpected kind: {:?}", other),
    }

    // 5. Stop tracing; further pushes are silently dropped and
    //    `capturing` flips to false.
    let (status, _) = http_request(&addr, "POST", "/v1/script/trace/stop", "{}");
    assert_eq!(status, 200);
    sink.push(suspend("e-dropped"));
    let (status, body) = http_request(&addr, "GET", "/v1/script/trace/drain?after_seq=4", "");
    assert_eq!(status, 200, "drain3 body: {body}");
    let drain = parse_drain(&body);
    assert!(!drain.capturing);
    assert!(drain.events.is_empty());
}

#[test]
fn trace_drain_reports_dropped_when_ring_wraps() {
    let (queue, consumer) = AgentCommandQueue::new();
    let sink = Arc::new(AgentTraceSink::new(2));
    let _worker = spawn_trace_worker(consumer, sink.clone());
    let server = AgentServer::start(AgentServerConfig::loopback(0), &queue, None)
        .expect("start agent server");
    let addr = server.local_addr();

    let (_, _) = http_request(&addr, "POST", "/v1/script/trace/start", "{}");
    for i in 0..5 {
        sink.push(suspend(&format!("ev{i}")));
    }

    let (status, body) = http_request(&addr, "GET", "/v1/script/trace/drain?after_seq=0", "");
    assert_eq!(status, 200, "drain body: {body}");
    let drain = parse_drain(&body);
    assert!(drain.dropped, "expected dropped=true; got: {body}");
    // Only the two most-recently pushed events survive in a cap-2 ring.
    assert_eq!(drain.events.len(), 2);
}
