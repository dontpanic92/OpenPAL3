//! PAL4-specific [`agent_server`] adapter.
//!
//! Combines the shared, game-agnostic [`AgentBridge`] (queue,
//! synthetic input, frame counters, pause/step cells, trace sink,
//! rendering engine slot) with the PAL4-specific
//! [`AgentTraceAdapter`] that forwards AngelScript-VM
//! [`TraceEvent`]s into the shared `AgentTraceSink` ring buffer.
//!
//! `Pal4AgentBridge` exists purely to bolt the PAL4 trace adapter
//! onto the shared bridge; it transparently delegates every other
//! field via [`Deref`](std::ops::Deref).
//!
//! Construct one via [`Pal4AgentBridge::new`] from the application
//! loader; install it on `Pal4Service` via `set_agent_bridge`.

use std::ops::Deref;
use std::rc::Rc;

use agent_server::AgentTraceSink;
use agent_server::protocol::{
    TraceBranchKind, TraceEventKindPayload, TraceEventPayload, TraceGlobalScope,
};
use radiance::input::SyntheticInputBridge;

use crate::agent_common::bridge::AgentBridge;
use crate::scripting::angelscript::{
    BranchKind, GlobalScope, TraceEvent, TraceEventKind, TraceSink,
};

// Re-export the shared default so existing call-sites importing it
// from this module keep compiling.
pub use crate::agent_common::bridge::DEFAULT_STEP_DT;

/// PAL4-flavoured [`AgentBridge`]: the shared bridge plus the
/// AngelScript [`AgentTraceAdapter`]. Held by the director and
/// `Pal4Service` as `Option<Rc<Pal4AgentBridge>>`.
pub struct Pal4AgentBridge {
    /// Shared, game-agnostic bridge state. Cloned freely; the
    /// listener thread and the dispatcher both reach the queue /
    /// trace sink through this.
    pub inner: Rc<AgentBridge>,
    /// VM-side trace adapter installed on `ScriptVm` whenever
    /// `trace_sink` is actively capturing. Held in an `Rc` because
    /// installation happens at most a handful of times per session
    /// (start/stop), not in the hot path.
    pub trace_adapter: Rc<AgentTraceAdapter>,
}

impl Pal4AgentBridge {
    /// Construct a fresh bridge wrapping `input_bridge`. The caller
    /// is responsible for sharing the same `Rc<RefCell<...>>` with
    /// `Pal4VmContext` so injected input actually reaches script
    /// consumers.
    pub fn new(input_bridge: Rc<std::cell::RefCell<SyntheticInputBridge>>) -> Self {
        let inner = Rc::new(AgentBridge::new(input_bridge));
        let trace_adapter = Rc::new(AgentTraceAdapter::new(inner.trace_sink.clone()));
        Self {
            inner,
            trace_adapter,
        }
    }
}

impl Deref for Pal4AgentBridge {
    type Target = AgentBridge;
    fn deref(&self) -> &AgentBridge {
        &self.inner
    }
}

/// VM-side trace sink that forwards every [`TraceEvent`] into the
/// agent-server's [`AgentTraceSink`] ring buffer.
///
/// Construct one via [`AgentTraceAdapter::new`] and install it on
/// `ScriptVm::set_trace_sink` via a `Rc<dyn TraceSink>` coercion.
/// The conversion is a straight one-to-one mapping; field renames
/// (e.g. `usize` → `u32` for the JSON wire form) happen here so the
/// VM-side enum stays free of agent-server concerns.
pub struct AgentTraceAdapter {
    sink: Rc<AgentTraceSink>,
}

impl AgentTraceAdapter {
    pub fn new(sink: Rc<AgentTraceSink>) -> Self {
        Self { sink }
    }

    pub fn sink(&self) -> &AgentTraceSink {
        &self.sink
    }
}

impl TraceSink for AgentTraceAdapter {
    fn record(&self, event: TraceEvent) {
        // The ring buffer assigns its own monotonic seq; the
        // VM-side seq is ignored on the wire. The payload's `seq`
        // field is overwritten by `AgentTraceSink::push`.
        self.sink.push(TraceEventPayload {
            seq: event.seq,
            kind: convert_kind(event.kind),
        });
    }
}

fn convert_kind(kind: TraceEventKind) -> TraceEventKindPayload {
    match kind {
        TraceEventKind::FnEnter {
            name,
            function_index,
            depth,
        } => TraceEventKindPayload::FnEnter {
            name,
            function_index: function_index as u32,
            depth: depth as u32,
        },
        TraceEventKind::FnExit { name, depth } => TraceEventKindPayload::FnExit {
            name,
            depth: depth as u32,
        },
        TraceEventKind::Branch {
            fn_name,
            pc,
            kind,
            operand,
            offset,
            taken,
        } => TraceEventKindPayload::Branch {
            fn_name,
            pc: pc as u32,
            branch: convert_branch(kind),
            operand,
            offset,
            taken,
        },
        TraceEventKind::CallSys {
            fn_name,
            pc,
            sysfn_index,
            sysfn_name,
            sp_before,
            sp_after,
            r1_after,
        } => TraceEventKindPayload::CallSys {
            fn_name,
            pc: pc as u32,
            sysfn_index: sysfn_index as u32,
            sysfn_name,
            sp_before: sp_before as u32,
            sp_after: sp_after as u32,
            r1_after,
        },
        TraceEventKind::GlobalRead {
            fn_name,
            pc,
            scope,
            slot,
            value,
        } => TraceEventKindPayload::GlobalRead {
            fn_name,
            pc: pc as u32,
            scope: convert_scope(scope),
            slot,
            value,
        },
        TraceEventKind::GlobalWrite {
            fn_name,
            pc,
            scope,
            slot,
            value,
        } => TraceEventKindPayload::GlobalWrite {
            fn_name,
            pc: pc as u32,
            scope: convert_scope(scope),
            slot,
            value,
        },
        TraceEventKind::Suspend { fn_name, pc } => TraceEventKindPayload::Suspend {
            fn_name,
            pc: pc as u32,
        },
    }
}

fn convert_branch(kind: BranchKind) -> TraceBranchKind {
    match kind {
        BranchKind::Jz => TraceBranchKind::Jz,
        BranchKind::Jnz => TraceBranchKind::Jnz,
        BranchKind::JsJgez => TraceBranchKind::JsJgez,
        BranchKind::JnsJlz => TraceBranchKind::JnsJlz,
        BranchKind::JpJlez => TraceBranchKind::JpJlez,
        BranchKind::JnpJgz => TraceBranchKind::JnpJgz,
    }
}

fn convert_scope(scope: GlobalScope) -> TraceGlobalScope {
    match scope {
        GlobalScope::Shared => TraceGlobalScope::Shared,
        GlobalScope::Module => TraceGlobalScope::Module,
    }
}
