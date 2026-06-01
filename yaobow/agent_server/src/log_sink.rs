//! Bounded ring-buffer [`log::Log`] implementation.
//!
//! Installed once via [`AgentLogSink::install`]; the agent server's
//! `/v1/log/tail` endpoint reads back records since a caller-supplied
//! sequence number. Each record carries a monotonic `seq` so clients
//! can detect dropped ranges when the ring wraps.
//!
//! Writes are lock-protected (`Mutex`) but cheap: a single push into a
//! `VecDeque<LogRecord>` of fixed capacity. Reads return owned copies.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use log::{Level, Log, Metadata, Record};

/// One buffered log record.
#[derive(Debug, Clone)]
pub struct LogRecord {
    /// Monotonic sequence, never reused.
    pub seq: u64,
    pub level: Level,
    pub target: String,
    pub message: String,
}

/// Inner ring-buffer state. Separate from the [`Log`] impl so the
/// transport can hold an `Arc<AgentLogSink>` for reads without
/// fighting the global logger borrow.
struct Ring {
    cap: usize,
    next_seq: u64,
    /// `true` once the buffer has rolled past the lowest unread
    /// record; cleared on every successful `tail` call that caught up.
    dropped: bool,
    records: VecDeque<LogRecord>,
}

impl Ring {
    fn new(cap: usize) -> Self {
        let cap = cap.max(1);
        Self {
            cap,
            next_seq: 1,
            dropped: false,
            records: VecDeque::with_capacity(cap),
        }
    }

    fn push(&mut self, level: Level, target: String, message: String) {
        let seq = self.next_seq;
        self.next_seq += 1;

        if self.records.len() == self.cap {
            self.records.pop_front();
            self.dropped = true;
        }

        self.records.push_back(LogRecord {
            seq,
            level,
            target,
            message,
        });
    }

    fn tail(&mut self, after_seq: u64, n: usize) -> (Vec<LogRecord>, u64, bool) {
        let mut out = Vec::new();
        // `next_seq` is the *next* sequence to be issued; the highest
        // currently-stored seq is `next_seq - 1`.
        let next = self.next_seq;
        let dropped = self.dropped && self.records.front().map_or(0, |r| r.seq) > after_seq + 1;

        for rec in &self.records {
            if rec.seq <= after_seq {
                continue;
            }
            if out.len() >= n {
                break;
            }
            out.push(rec.clone());
        }

        // Reset `dropped` once the caller has caught past the lowest
        // surviving record (so a steady tail loop stops flagging it).
        if !dropped {
            self.dropped = false;
        }

        (out, next, dropped)
    }
}

/// `log::Log` adapter that fans records into a bounded ring buffer.
pub struct AgentLogSink {
    ring: Mutex<Ring>,
    /// Maximum level passed through `enabled`. Configured at install
    /// time; defaults to `Trace` so the sink mirrors the host
    /// application's filter.
    max_level: log::LevelFilter,
}

impl AgentLogSink {
    /// New ring with the given capacity (clamped to at least 16 to
    /// keep production tails meaningful; unit tests in
    /// [`crate::log_sink`] exercise the inner ring at smaller sizes).
    pub fn new(capacity: usize) -> Self {
        Self {
            ring: Mutex::new(Ring::new(capacity.max(16))),
            max_level: log::LevelFilter::Trace,
        }
    }

    /// Stash this sink in a process-global slot and return the leaked
    /// `&'static` reference, **without** registering it as the global
    /// [`log`] logger. Used when the host wants to fan records into
    /// this sink alongside another logger (e.g. `simple_logger`) via
    /// its own tee adapter — the tee installs itself, then forwards
    /// each record to the leaked sink via [`Self::record_external`].
    ///
    /// Subsequent calls return the previously stored sink (the new
    /// instance is dropped). Pair with [`Self::global`] from other
    /// crates that need a handle to the live sink.
    pub fn leak(self) -> &'static AgentLogSink {
        STORED.get_or_init(|| self)
    }

    /// Read-only access to the process-global sink installed by an
    /// earlier [`Self::install`] / [`Self::leak`] call. Returns
    /// `None` when no sink has been stashed yet.
    pub fn global() -> Option<&'static AgentLogSink> {
        STORED.get()
    }

    /// Install this sink as the global [`log`] logger. Returns
    /// `Err(_)` if a logger is already installed (the host's
    /// `simple_logger` install runs first when the binary boots; in
    /// that case callers should fall back to a passive sink that the
    /// host explicitly forwards into).
    pub fn install(self) -> Result<&'static AgentLogSink, log::SetLoggerError> {
        let leaked = self.leak();
        log::set_logger(leaked).map(|()| {
            log::set_max_level(leaked.max_level);
            leaked
        })
    }

    /// Read at most `n` records with `seq > after_seq`. Returns the
    /// new `next_seq` cursor and a `dropped` flag for the caller to
    /// surface if the requested range was already evicted.
    pub fn tail(&self, after_seq: u64, n: usize) -> (Vec<LogRecord>, u64, bool) {
        let mut ring = self.ring.lock().expect("agent log sink mutex poisoned");
        ring.tail(after_seq, n)
    }

    /// Append a record bypassing the `log::Log` plumbing — used when
    /// the host already has a global logger and forwards records into
    /// the sink manually.
    pub fn record_external(&self, level: Level, target: &str, message: &str) {
        let mut ring = self.ring.lock().expect("agent log sink mutex poisoned");
        ring.push(level, target.to_string(), message.to_string());
    }
}

/// Process-global slot for the leaked sink. Shared by every crate
/// that calls [`AgentLogSink::leak`] / [`AgentLogSink::global`] /
/// [`AgentLogSink::install`], so a tee installed by the binary and a
/// passive lookup from `application.rs` see the same instance.
static STORED: OnceLock<AgentLogSink> = OnceLock::new();

impl Log for AgentLogSink {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.max_level
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let msg = format!("{}", record.args());
        let target = record.target().to_string();
        let mut ring = self.ring.lock().expect("agent log sink mutex poisoned");
        ring.push(record.level(), target, msg);
    }

    fn flush(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_drops_oldest_when_full() {
        let mut ring = Ring::new(4);
        for i in 0..6 {
            ring.push(Level::Info, "t".into(), format!("m{i}"));
        }
        assert_eq!(ring.records.len(), 4);
        assert_eq!(ring.records.front().unwrap().seq, 3);
        assert_eq!(ring.records.back().unwrap().seq, 6);
        assert!(ring.dropped);
    }

    #[test]
    fn tail_returns_records_after_cursor_and_respects_cap() {
        let mut ring = Ring::new(8);
        for i in 0..6 {
            ring.push(Level::Info, "t".into(), format!("m{i}"));
        }
        let (out, next_seq, dropped) = ring.tail(2, 100);
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].seq, 3);
        assert_eq!(next_seq, 7);
        assert!(!dropped);

        let (out, _, _) = ring.tail(0, 2);
        assert_eq!(out.len(), 2);
        assert_eq!(out.iter().map(|r| r.seq).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn dropped_flag_set_when_caller_lags_too_far() {
        let mut ring = Ring::new(2);
        for i in 0..5 {
            ring.push(Level::Info, "t".into(), format!("m{i}"));
        }
        // Caller last saw seq=1, but the lowest stored is now 4.
        let (out, next, dropped) = ring.tail(1, 100);
        assert!(dropped);
        assert_eq!(next, 6);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].seq, 4);
    }
}
