//! PAL4-specific [`agent_server`] adapter.
//!
//! Centralizes the cross-thread state that the embedded HTTP agent
//! server needs to drive a running PAL4 session:
//!
//! * A [`AgentCommandQueue`] whose `Sender` is cloned into the HTTP
//!   listener thread and whose `Receiver` is drained on the game
//!   thread (inside [`OpenPAL4Director::update`]).
//! * A [`SyntheticInputBridge`] that overlays agent-driven key / axis
//!   events on top of the real input engine. Replaces the `input`
//!   handle handed to `Pal4AppContext` so every consumer (scripts,
//!   actor controllers, the director itself) sees the merged view.
//! * A small bag of [`Cell`](std::cell::Cell)s exposing pause /
//!   fixed-step / fps state to the director and back.
//!
//! `Pal4AgentBridge` is *only* state — the actual command dispatch
//! (`AgentCommand -> AgentResponse`) lives in
//! [`super::director::dispatch_agent_command`] so it can call into the
//! director's `vm` / `Pal4AppContext` without an extra interior-
//! mutability hop.
//!
//! ## Threading
//!
//! Everything in this module that crosses the HTTP↔game boundary
//! goes through `AgentCommandQueue` (which uses `std::sync::mpsc`).
//! All `Rc<RefCell<...>>` handles stay on the game thread.

use std::cell::Cell;
use std::rc::Rc;

use agent_server::protocol::{
    TraceBranchKind, TraceEventKindPayload, TraceEventPayload, TraceGlobalScope,
};
use agent_server::{AgentCommandConsumer, AgentCommandQueue, AgentTraceSink};
use radiance::input::SyntheticInputBridge;
use radiance::rendering::RenderingEngine;

use crate::scripting::angelscript::{
    BranchKind, GlobalScope, TraceEvent, TraceEventKind, TraceSink,
};

/// Default per-frame delta used by `/v1/time/step` when the caller
/// doesn't provide one — matches the engine's nominal 60 Hz target.
pub const DEFAULT_STEP_DT: f32 = 1.0 / 60.0;

/// Cross-thread bundle of state the agent surface manipulates.
///
/// Held by the director as an `Option<Rc<Pal4AgentBridge>>` (the
/// `None` arm covers the normal "no agent" build). When present, the
/// HTTP listener thread is alive and the director runs the per-frame
/// drain + pause / step machinery defined below.
pub struct Pal4AgentBridge {
    /// Producer half of the command pipe. Cloned into the HTTP
    /// listener via [`crate::openpal4::director::OpenPAL4Director`]'s
    /// boot path.
    pub queue: AgentCommandQueue,

    /// Consumer side, drained by the director once per `update`.
    /// Stored in a `RefCell` so the borrowing director can take it
    /// out for the brief drain window without re-entering its own
    /// agent state.
    pub consumer: std::cell::RefCell<Option<AgentCommandConsumer>>,

    /// Synthetic-input overlay handed to `Pal4AppContext` so taps /
    /// holds / axes injected via `/v1/input/*` are visible to the
    /// scripts that poll `input.get_key_state(...)`.
    pub input_bridge: Rc<std::cell::RefCell<SyntheticInputBridge>>,

    /// Execution-trace ring shared with the VM via
    /// [`AgentTraceAdapter`]. Lives in the bridge so the director's
    /// `TraceStart` / `TraceStop` / `TraceDrain` handlers can poke
    /// it without going through the VM.
    pub trace_sink: Rc<AgentTraceSink>,

    /// VM-side trace adapter installed on `ScriptVm` whenever
    /// [`Self::trace_sink`] is actively capturing. Held in a `Cell`-
    /// less `Rc` because installation happens at most a handful of
    /// times per session (start/stop), not in the hot path.
    pub trace_adapter: Rc<AgentTraceAdapter>,

    /// Optional handle to the live rendering engine. When present, the
    /// director's `/v1/screenshot` handler reads back the last
    /// presented swapchain image via
    /// [`RenderingEngine::capture_last_frame`]. `None` in tests that
    /// don't spin up Vulkan and in headless boots that haven't wired
    /// a readback-capable backend.
    pub rendering_engine: std::cell::RefCell<Option<Rc<std::cell::RefCell<dyn RenderingEngine>>>>,

    /// Monotonic frame counter — incremented exactly once per game
    /// tick that actually advanced (real + stepped).
    pub frame: Cell<u64>,

    /// `true` while the director is in fixed-step mode (no automatic
    /// advance per real frame; only `request_steps` ticks the world).
    pub paused: Cell<bool>,

    /// Remaining `step` frames requested by the latest
    /// `/v1/time/step` command. Decremented by the director until it
    /// hits zero.
    pub requested_steps: Cell<u32>,

    /// Per-frame `dt` to use for stepped frames. `0.0` ≡ "use
    /// [`DEFAULT_STEP_DT`]".
    pub requested_dt: Cell<f32>,

    /// Latest published FPS / per-frame `dt`. Mirrors the values the
    /// debug overlay already maintains; lets the agent snapshot
    /// expose them without re-doing the smoothing math.
    pub fps_display: Cell<f32>,
    pub dt_display: Cell<f32>,
}

impl Pal4AgentBridge {
    /// Construct a fresh bridge wrapping `input_bridge`. The caller
    /// is responsible for sharing the same `Rc<RefCell<...>>` with
    /// `Pal4AppContext` so injected input actually reaches script
    /// consumers.
    pub fn new(input_bridge: Rc<std::cell::RefCell<SyntheticInputBridge>>) -> Self {
        let (queue, consumer) = AgentCommandQueue::new();
        let trace_sink = Rc::new(AgentTraceSink::with_default_capacity());
        let trace_adapter = Rc::new(AgentTraceAdapter::new(trace_sink.clone()));
        Self {
            queue,
            consumer: std::cell::RefCell::new(Some(consumer)),
            input_bridge,
            trace_sink,
            trace_adapter,
            rendering_engine: std::cell::RefCell::new(None),
            frame: Cell::new(0),
            paused: Cell::new(false),
            requested_steps: Cell::new(0),
            requested_dt: Cell::new(0.0),
            fps_display: Cell::new(0.0),
            dt_display: Cell::new(0.0),
        }
    }

    /// Install the rendering-engine handle used by `/v1/screenshot`.
    /// Idempotent — passing a new handle replaces the previous one.
    pub fn set_rendering_engine(&self, engine: Rc<std::cell::RefCell<dyn RenderingEngine>>) {
        *self.rendering_engine.borrow_mut() = Some(engine);
    }

    /// Effective per-step `dt`, accounting for the "0 means default"
    /// convention on [`Self::requested_dt`].
    pub fn effective_step_dt(&self) -> f32 {
        let dt = self.requested_dt.get();
        if dt > 0.0 { dt } else { DEFAULT_STEP_DT }
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
