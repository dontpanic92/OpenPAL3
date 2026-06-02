//! Structured execution trace produced by the AngelScript VM.
//!
//! The [`TraceSink`] trait is the integration point: install one via
//! [`ScriptVm::set_trace_sink`](super::ScriptVm::set_trace_sink) and
//! every branch, sysfn call, and global read/write the VM executes is
//! delivered as a [`TraceEvent`]. When no sink is installed the VM
//! skips the recording entirely (single `Option::is_none()` check on
//! the hot path), so the unobserved cost is zero beyond the existing
//! interpreter loop.
//!
//! The producer side (the VM) emits owned data — interned function
//! names as `String`, snapshot register values — so the consumer can
//! cross thread/queue boundaries (e.g. push events into the
//! `agent_server` ring buffer) without retaining VM borrows.
//!
//! This module deliberately stays free of dependencies on the agent
//! server crate: a downstream adapter (typically in
//! `yaobow/shared/src/openpal4/agent.rs`) implements [`TraceSink`]
//! against whatever transport it wants. Tests in this crate use a
//! simple `Mutex<Vec<_>>` sink.

/// Sink for trace events emitted by [`ScriptVm`](super::ScriptVm).
///
/// Implementations must be cheap on the hot path — the VM calls
/// [`Self::record`] from inside its opcode dispatch loop. A typical
/// implementation pushes the event onto a bounded ring buffer (see
/// `agent_server`'s log-sink design as a template).
pub trait TraceSink {
    /// Called once per traced VM event. The `seq` field on `event`
    /// is allocated monotonically by the VM; consumers may rely on
    /// it for ordering and gap detection.
    fn record(&self, event: TraceEvent);
}

/// One event in the VM execution trace.
#[derive(Debug, Clone)]
pub struct TraceEvent {
    /// Monotonic sequence number. Assigned by the VM at emission
    /// time; consumers use it as a cursor.
    pub seq: u64,
    pub kind: TraceEventKind,
}

/// Per-event payload. Variants mirror the VM opcodes the planner
/// needs to make plot-progression decisions.
#[derive(Debug, Clone)]
pub enum TraceEventKind {
    /// `call <fn>` — pushed a new frame onto the script call stack.
    FnEnter {
        /// Name of the entered function. May be empty if the module's
        /// function table didn't carry a name for the slot.
        name: String,
        /// Index into the owning [`ScriptModule::functions`](super::module::ScriptModule).
        function_index: usize,
        /// Depth *after* the push (i.e. `call_stack.len()` once the
        /// new frame is active).
        depth: usize,
    },

    /// `ret` — popped the active frame; the previous frame (if any)
    /// is now executing.
    FnExit {
        name: String,
        /// Depth *before* the pop, so subtracting one gives the
        /// resumed frame's depth.
        depth: usize,
    },

    /// Conditional jump opcode. `taken` is the runtime outcome.
    Branch {
        fn_name: String,
        /// PC of the branch instruction itself (start of the
        /// 4-byte op + 4-byte offset operand in `function.inst`).
        pc: usize,
        kind: BranchKind,
        /// Top-of-stack i32 the branch consumed before deciding.
        /// For the multi-op branches (`js_jgez`, …) this is the
        /// raw popped value before the sign test.
        operand: i32,
        /// Branch offset encoded in the instruction stream
        /// (signed, in bytes relative to the post-fetch PC).
        offset: i32,
        taken: bool,
    },

    /// System-function (`callsys`) invocation. Captures pre-call
    /// stack-pointer (for arg-frame size inference) and the post-
    /// call return register, which is where most `gi*` functions
    /// stash their `int` return value.
    CallSys {
        fn_name: String,
        pc: usize,
        sysfn_index: usize,
        sysfn_name: String,
        sp_before: usize,
        sp_after: usize,
        /// Post-call value of register `r1`. The legacy `gi*` ABI
        /// uses `r1` for the lone return value; `0` is the common
        /// "void / unused" sentinel.
        r1_after: u32,
    },

    /// Global-variable read (`rdga4` / `pga`).
    GlobalRead {
        fn_name: String,
        pc: usize,
        scope: GlobalScope,
        slot: u32,
        value: u32,
    },

    /// Global-variable write (`movga4`).
    GlobalWrite {
        fn_name: String,
        pc: usize,
        scope: GlobalScope,
        slot: u32,
        value: u32,
    },

    /// Cooperative-yield point (`Suspend` opcode). Signals "engine
    /// is going to let other work run before this function resumes".
    Suspend { fn_name: String, pc: usize },
}

/// Kind of conditional jump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchKind {
    Jz,
    Jnz,
    /// Composite "jump if sign / jump if greater-or-equal-zero".
    JsJgez,
    /// Composite "jump if not-sign / jump if less-than-zero".
    JnsJlz,
    /// Composite "jump if positive / jump if less-or-equal-zero".
    JpJlez,
    /// Composite "jump if not-positive / jump if greater-than-zero".
    JnpJgz,
}

/// Which global-variable namespace the read/write referenced.
///
/// AngelScript's `rdga4` / `movga4` encode shared globals (those
/// also visible via `ScriptGlobalContext::get_global`, the same
/// array surfaced by `/v1/script/globals`) as negative indices;
/// positive indices reference module-local globals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalScope {
    /// Shared / "plot-flag" global (negative-index decoded as
    /// `-index - 1`).
    Shared,
    /// Module-local global (positive index, indexes
    /// `ScriptModule::globals`).
    Module,
}

#[cfg(test)]
pub mod test_support {
    //! Tiny in-memory sink used by the VM unit tests.
    use std::sync::Mutex;

    use super::{TraceEvent, TraceSink};

    #[derive(Default)]
    pub struct VecSink {
        events: Mutex<Vec<TraceEvent>>,
    }

    impl VecSink {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn drain(&self) -> Vec<TraceEvent> {
            std::mem::take(&mut *self.events.lock().expect("vec-sink mutex"))
        }

        pub fn snapshot(&self) -> Vec<TraceEvent> {
            self.events.lock().expect("vec-sink mutex").clone()
        }
    }

    impl TraceSink for VecSink {
        fn record(&self, event: TraceEvent) {
            self.events.lock().expect("vec-sink mutex").push(event);
        }
    }
}
