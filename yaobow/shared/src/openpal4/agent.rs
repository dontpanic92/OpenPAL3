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

use agent_server::{AgentCommandConsumer, AgentCommandQueue};
use radiance::input::SyntheticInputBridge;
use radiance::rendering::RenderingEngine;

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
        Self {
            queue,
            consumer: std::cell::RefCell::new(Some(consumer)),
            input_bridge,
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
        if dt > 0.0 {
            dt
        } else {
            DEFAULT_STEP_DT
        }
    }
}
