//! Cross-thread bridge state used by every per-game agent adapter.
//!
//! This is the **game-agnostic** chunk of what used to be
//! `Pal4AgentBridge`: the command queue, synthetic-input overlay,
//! pause / fixed-step cells, FPS EMA, trace-ring slot, and the
//! optional rendering-engine handle used by `/v1/screenshot`. PAL3,
//! PAL4 and any future game share one instance per running session.
//!
//! ## What does *not* live here
//!
//! * VM-side trace adapters (AngelScript for PAL4, future SceVm
//!   adapter for PAL3) — they are game-specific and stay next to the
//!   VM they bridge.
//! * Snapshot construction and the `AgentCommand` -> `AgentResponse`
//!   dispatch — handled by `shared::openpalN::agent` modules that
//!   know the per-game state shape.
//!
//! ## Threading
//!
//! Everything that crosses the HTTP↔game boundary goes through the
//! `AgentCommandQueue` (`std::sync::mpsc`). The `Rc<RefCell<...>>`
//! handles inside `AgentBridge` stay on the game thread.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use agent_server::{AgentCommandConsumer, AgentCommandQueue, AgentTraceSink};
use radiance::input::SyntheticInputBridge;
use radiance::rendering::RenderingEngine;

/// Default per-frame delta used by `/v1/time/step` when the caller
/// doesn't provide one — matches the engine's nominal 60 Hz target.
pub const DEFAULT_STEP_DT: f32 = 1.0 / 60.0;

/// Cross-thread bundle of state the agent surface manipulates.
///
/// Held by each per-game service as an `Option<Rc<AgentBridge>>`
/// (the `None` arm covers the normal "no agent" build). When present,
/// the HTTP listener thread is alive and the service runs the
/// per-frame drain + pause / step machinery defined below.
pub struct AgentBridge {
    /// Producer half of the command pipe. Cloned into the HTTP
    /// listener via the boot helper in [`super::launch`].
    pub queue: AgentCommandQueue,

    /// Consumer side, drained by the game's service once per
    /// `pump_agent`. Stored in a `RefCell` so the borrowing
    /// dispatcher can take it out for the brief drain window
    /// without re-entering its own agent state.
    pub consumer: RefCell<Option<AgentCommandConsumer>>,

    /// Synthetic-input overlay handed to the per-game VM context so
    /// taps / holds / axes injected via `/v1/input/*` are visible to
    /// the scripts that poll `input.get_key_state(...)`.
    pub input_bridge: Rc<RefCell<SyntheticInputBridge>>,

    /// Execution-trace ring shared with the per-game VM via that
    /// game's `TraceAdapter`. Lives on the bridge so the
    /// `TraceStart` / `TraceStop` / `TraceDrain` handlers can poke
    /// it without going through the VM.
    pub trace_sink: Rc<AgentTraceSink>,

    /// Optional handle to the live rendering engine. When present,
    /// the `/v1/screenshot` handler reads back the last presented
    /// swapchain image via [`RenderingEngine::capture_last_frame`].
    /// `None` in tests that don't spin up Vulkan and in headless
    /// boots that haven't wired a readback-capable backend.
    pub rendering_engine: RefCell<Option<Rc<RefCell<dyn RenderingEngine>>>>,

    /// Monotonic frame counter — incremented exactly once per game
    /// tick that actually advanced (real + stepped).
    pub frame: Cell<u64>,

    /// `true` while the dispatcher is in fixed-step mode (no
    /// automatic advance per real frame; only `request_steps` ticks
    /// the world).
    pub paused: Cell<bool>,

    /// Remaining `step` frames requested by the latest
    /// `/v1/time/step` command. Decremented by the dispatcher until
    /// it hits zero.
    pub requested_steps: Cell<u32>,

    /// Per-frame `dt` to use for stepped frames. `0.0` ≡ "use
    /// [`DEFAULT_STEP_DT`]".
    pub requested_dt: Cell<f32>,

    /// Whether plot fast-forward is enabled. Per-game VMs read this
    /// to skip scripted waits, dialog waits, movie playback, etc.
    pub fast_forward: Cell<bool>,

    /// Whether the free-fly debug camera is enabled. Set by the agent
    /// surface (`/v1/camera/debug`); read by the per-game director,
    /// which freezes the plot and drives the scene camera manually
    /// while it is `true`. Generic so any game can honor it.
    pub debug_cam: Cell<bool>,

    /// Latest published FPS / per-frame `dt`. Mirrors the values the
    /// debug overlay already maintains; lets the agent snapshot
    /// expose them without re-doing the smoothing math.
    pub fps_display: Cell<f32>,
    pub dt_display: Cell<f32>,
}

impl AgentBridge {
    /// Construct a fresh bridge wrapping `input_bridge`. The caller
    /// is responsible for sharing the same `Rc<RefCell<...>>` with
    /// the per-game VM context so injected input actually reaches
    /// script consumers.
    pub fn new(input_bridge: Rc<RefCell<SyntheticInputBridge>>) -> Self {
        let (queue, consumer) = AgentCommandQueue::new();
        let trace_sink = Rc::new(AgentTraceSink::with_default_capacity());
        Self {
            queue,
            consumer: RefCell::new(Some(consumer)),
            input_bridge,
            trace_sink,
            rendering_engine: RefCell::new(None),
            frame: Cell::new(0),
            paused: Cell::new(false),
            requested_steps: Cell::new(0),
            requested_dt: Cell::new(0.0),
            fast_forward: Cell::new(false),
            debug_cam: Cell::new(false),
            fps_display: Cell::new(0.0),
            dt_display: Cell::new(0.0),
        }
    }

    /// Install the rendering-engine handle used by `/v1/screenshot`.
    /// Idempotent — passing a new handle replaces the previous one.
    pub fn set_rendering_engine(&self, engine: Rc<RefCell<dyn RenderingEngine>>) {
        *self.rendering_engine.borrow_mut() = Some(engine);
    }

    /// Effective per-step `dt`, accounting for the "0 means default"
    /// convention on [`Self::requested_dt`].
    pub fn effective_step_dt(&self) -> f32 {
        let dt = self.requested_dt.get();
        if dt > 0.0 { dt } else { DEFAULT_STEP_DT }
    }

    /// Pause / single-step **policy** — the generic time-control rule
    /// the agent surface enforces, kept on the bridge so **any** game
    /// can gate its own simulation tick through it instead of
    /// re-implementing it.
    ///
    /// Returns `(advance, effective_dt)`:
    /// * not paused → `(true, delta_sec)` (run at the real frame rate);
    /// * paused with no pending steps → `(false, 0.0)` (freeze — the
    ///   caller must skip its simulation tick this frame);
    /// * paused with pending steps → consumes one step and returns
    ///   `(true, effective_step_dt())` (advance exactly one fixed step).
    pub fn effective_dt(&self, delta_sec: f32) -> (bool, f32) {
        if !self.paused.get() {
            return (true, delta_sec);
        }
        let pending = self.requested_steps.get();
        if pending == 0 {
            return (false, 0.0);
        }
        self.requested_steps.set(pending - 1);
        (true, self.effective_step_dt())
    }

    /// Per-frame agent **telemetry** published once per frame in every
    /// game by the app-lifetime dispatcher: advance the monotonic
    /// frame counter and publish the latest `dt` / smoothed FPS for
    /// `/v1/state`.
    ///
    /// Note: this does **not** clear synthetic-input edges — that
    /// must run *after* the active game polls input for the frame,
    /// so it stays with the caller that owns the input-polling tick.
    pub fn publish_frame_telemetry(&self, dt: f32) {
        self.frame.set(self.frame.get().saturating_add(1));
        self.dt_display.set(dt);
        let inst_fps = if dt > 1e-4 { 1.0 / dt } else { 0.0 };
        let prev = self.fps_display.get();
        let fps = if prev <= 0.0 {
            inst_fps
        } else {
            prev * 0.9 + inst_fps * 0.1
        };
        self.fps_display.set(fps);
    }
}
