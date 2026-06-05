//! Bounded ring buffer for VM execution trace events.
//!
//! Mirrors [`crate::log_sink`]'s design exactly — a [`Mutex`]ed
//! `VecDeque` with monotonic sequence numbers and a `dropped` flag —
//! but stores [`TraceEventPayload`]s instead of log records.
//!
//! The ring is intentionally type-erased from the VM-side `TraceSink`
//! trait that lives in `shared::scripting::angelscript::trace`: a
//! per-game adapter (in `yaobow/shared/src/openpal4/agent.rs`)
//! implements that trait by converting each event to a payload and
//! calling [`AgentTraceSink::push`] here. This keeps `agent_server`
//! free of any dependency on the VM crate.

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::protocol::TraceEventPayload;

/// Default ring capacity if a [`TraceStart`](crate::protocol::AgentCommand::TraceStart)
/// caller doesn't override it. ~64k events covers several seconds of
/// busy scripted play.
pub const DEFAULT_TRACE_CAPACITY: usize = 65_536;

/// Result of a [`AgentTraceSink::drain`] call.
pub struct TraceDrainResult {
    pub events: Vec<TraceEventPayload>,
    pub next_seq: u64,
    pub dropped: bool,
}

struct Ring {
    cap: usize,
    next_seq: u64,
    dropped: bool,
    events: VecDeque<TraceEventPayload>,
}

impl Ring {
    fn new(cap: usize) -> Self {
        let cap = cap.max(1);
        Self {
            cap,
            next_seq: 1,
            dropped: false,
            events: VecDeque::with_capacity(cap.min(4096)),
        }
    }

    fn push(&mut self, mut event: TraceEventPayload) -> u64 {
        let seq = self.next_seq;
        event.seq = seq;
        self.next_seq = seq.wrapping_add(1);

        if self.events.len() == self.cap {
            self.events.pop_front();
            self.dropped = true;
        }
        self.events.push_back(event);
        seq
    }

    fn drain(&mut self, after_seq: u64, n: usize) -> TraceDrainResult {
        let mut out = Vec::new();
        let next = self.next_seq;
        // A caller fell behind only if the lowest stored seq is past
        // their cursor (i.e. they missed the [after_seq+1 ..] prefix).
        let dropped =
            self.dropped && self.events.front().map_or(0, |e| e.seq) > after_seq.saturating_add(1);

        for ev in &self.events {
            if ev.seq <= after_seq {
                continue;
            }
            if out.len() >= n {
                break;
            }
            out.push(ev.clone());
        }

        if !dropped {
            // Caller has either caught up or asked for nothing past
            // the eviction point — clear the warning so steady tails
            // stop flagging it.
            self.dropped = false;
        }

        TraceDrainResult {
            events: out,
            next_seq: next,
            dropped,
        }
    }

    fn reset(&mut self, cap: Option<usize>) {
        if let Some(c) = cap {
            self.cap = c.max(1);
        }
        self.next_seq = 1;
        self.dropped = false;
        self.events.clear();
    }
}

/// Trace-event sink consumed by the per-game VM adapter.
pub struct AgentTraceSink {
    ring: Mutex<Ring>,
    capturing: std::sync::atomic::AtomicBool,
}

impl AgentTraceSink {
    pub fn new(capacity: usize) -> Self {
        Self {
            ring: Mutex::new(Ring::new(capacity)),
            capturing: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Default-capacity instance. Equivalent to
    /// `AgentTraceSink::new(DEFAULT_TRACE_CAPACITY)`.
    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_TRACE_CAPACITY)
    }

    /// Begin capturing. The VM adapter checks [`Self::is_capturing`]
    /// on each event so events emitted while capturing is off are
    /// dropped on the producer side (zero ring contention).
    ///
    /// When `reset` is true (the common case) the buffer is cleared
    /// and the monotonic sequence counter restarts at 0.
    pub fn start(&self, reset: bool, capacity: Option<usize>) {
        if reset {
            let mut ring = self.ring.lock().expect("trace ring poisoned");
            ring.reset(capacity);
        } else if let Some(cap) = capacity {
            let mut ring = self.ring.lock().expect("trace ring poisoned");
            ring.cap = cap.max(1);
        }
        self.capturing
            .store(true, std::sync::atomic::Ordering::Release);
    }

    /// Stop capturing. Already-buffered events stay drainable.
    pub fn stop(&self) {
        self.capturing
            .store(false, std::sync::atomic::Ordering::Release);
    }

    pub fn is_capturing(&self) -> bool {
        self.capturing.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Push one event into the ring. Cheaply ignored when capturing
    /// is off, so the producer can call it unconditionally per
    /// emitted event (the [`AtomicBool::load`] is a single
    /// uncontended memory read on the hot path).
    pub fn push(&self, event: TraceEventPayload) {
        if !self.is_capturing() {
            return;
        }
        let mut ring = self.ring.lock().expect("trace ring poisoned");
        ring.push(event);
    }

    pub fn drain(&self, after_seq: u64, n: usize) -> TraceDrainResult {
        let mut ring = self.ring.lock().expect("trace ring poisoned");
        ring.drain(after_seq, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{TraceEventKindPayload, TraceEventPayload};

    fn evt(kind: TraceEventKindPayload) -> TraceEventPayload {
        TraceEventPayload { seq: 0, kind }
    }

    fn suspend(name: &str) -> TraceEventKindPayload {
        TraceEventKindPayload::Suspend {
            fn_name: name.to_string(),
            pc: 0,
        }
    }

    #[test]
    fn push_assigns_monotonic_seq_starting_at_one() {
        let sink = AgentTraceSink::new(8);
        sink.start(true, None);
        for i in 0..3 {
            sink.push(evt(suspend(&format!("f{}", i))));
        }
        let r = sink.drain(0, 100);
        assert!(!r.dropped);
        let seqs: Vec<u64> = r.events.iter().map(|e| e.seq).collect();
        assert_eq!(seqs, vec![1, 2, 3]);
        assert_eq!(r.next_seq, 4);
    }

    #[test]
    fn capturing_off_drops_events() {
        let sink = AgentTraceSink::new(8);
        // never called start
        sink.push(evt(suspend("orphan")));
        let r = sink.drain(0, 100);
        assert!(r.events.is_empty());
        // `next_seq` reads the initial cursor; the ring starts at 1
        // to match the log-sink convention so `after_seq=0` returns
        // every observed event.
        assert_eq!(r.next_seq, 1);
        assert!(!sink.is_capturing());
    }

    #[test]
    fn stop_keeps_buffered_events_drainable() {
        let sink = AgentTraceSink::new(8);
        sink.start(true, None);
        sink.push(evt(suspend("a")));
        sink.push(evt(suspend("b")));
        sink.stop();
        // Post-stop pushes are dropped, but the buffered ones remain.
        sink.push(evt(suspend("c-ignored")));
        let r = sink.drain(0, 100);
        assert_eq!(r.events.len(), 2);
        assert!(!sink.is_capturing());
    }

    #[test]
    fn ring_wrap_marks_dropped() {
        let sink = AgentTraceSink::new(2);
        sink.start(true, None);
        for i in 0..5 {
            sink.push(evt(suspend(&format!("f{}", i))));
        }
        // Pushes use seqs 1..=5. Caller cursor is 0 but lowest
        // stored is seq=4 -> dropped flag set.
        let r = sink.drain(0, 100);
        assert!(r.dropped);
        assert_eq!(r.next_seq, 6);
        let seqs: Vec<u64> = r.events.iter().map(|e| e.seq).collect();
        assert_eq!(seqs, vec![4, 5]);
    }

    #[test]
    fn restart_resets_sequence_counter() {
        let sink = AgentTraceSink::new(8);
        sink.start(true, None);
        sink.push(evt(suspend("a")));
        sink.start(true, None);
        sink.push(evt(suspend("b")));
        let r = sink.drain(0, 100);
        let seqs: Vec<u64> = r.events.iter().map(|e| e.seq).collect();
        assert_eq!(seqs, vec![1]);
        assert_eq!(r.next_seq, 2);
    }
}
