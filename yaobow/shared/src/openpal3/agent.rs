//! PAL3-specific [`agent_server`] adapter.
//!
//! The shared, game-agnostic plumbing (queue, synthetic input,
//! pause/step cells, trace ring, rendering engine) lives in
//! [`crate::agent_common::AgentBridge`]. This module:
//!
//! * Builds [`StateSnapshot`]s from PAL3 state (`AdventureDirector`,
//!   [`SceVm`](crate::scripting::sce::vm::SceVm),
//!   [`GlobalState`](crate::openpal3::states::global_state::GlobalState),
//!   [`PersistentState`](crate::openpal3::states::persistent_state::PersistentState)).
//! * Dispatches [`AgentCommand`]s to the appropriate PAL3 surface
//!   (synthetic input → bridge, teleport → director, save/load → service).
//!
//! Unsupported endpoints (no clean PAL3 mapping today) return
//! `AgentResponse::Error { kind: NotImplemented }` so external
//! drivers can probe and fall back. See `docs/agent_interface.md`
//! for the per-endpoint matrix.
//!
//! The dispatcher is invoked from `Pal3Service::pump_agent`; it never
//! crosses the HTTP↔game thread boundary directly (everything goes
//! through the shared [`AgentCommandQueue`]).

use std::rc::Rc;

use agent_server::protocol::{
    AgentCommand, AgentError, AgentResponse, AxisInputParams, DialogSnapshot, KeyAction,
    KeyInputParams, ScreenshotResponse, ScriptGlobalsParams, ScriptGlobalsResponse, SlotParams,
    StateSnapshot, StatusMenuParams, StepTimeParams, TeleportParams,
};
use crosscom::ComRc;
use radiance::comdef::ISceneManager;
use radiance::input::{Axis, Key};
use radiance::math::Vec3;

use crate::agent_common::AgentBridge;
use crate::openpal3::directors::AdventureDirector;

/// Default size of the dense window returned by `/v1/script/globals`
/// when the caller omits `limit`. PAL3 globals are written sparsely
/// by the SCE VM, so the dense window is informative even past the
/// last populated slot.
const DEFAULT_GLOBALS_WINDOW: usize = 256;

/// Maximum globals slot we'll probe when reporting the array length.
/// PAL3's SCE scripts use signed 16-bit var indices; bounding the
/// probe at 1024 covers every observed global and keeps the snapshot
/// path O(N) with a fixed cap.
const GLOBALS_PROBE_MAX: i16 = 1024;

/// Stitched together once per command by `Pal3Service::pump_agent`.
///
/// Borrows are short-lived (one command's worth) so we don't hold
/// engine borrows across the dispatcher; the service is responsible
/// for cloning out the `SceneManager` handle ahead of time.
pub struct Pal3DispatchCtx<'a> {
    pub bridge: &'a Rc<AgentBridge>,
    /// Active adventure director, if one is installed. `None` while
    /// the start menu / title is up.
    pub director: Option<&'a AdventureDirector>,
    /// Live scene manager (cloned once per pump_agent).
    pub scene_manager: ComRc<ISceneManager>,
}

/// Dispatch a single [`AgentCommand`] against the supplied PAL3
/// context. The reply is wired straight back to the HTTP client by
/// the surrounding `pump_agent` loop.
pub fn dispatch_pal3_command(ctx: &Pal3DispatchCtx, command: AgentCommand) -> AgentResponse {
    use AgentCommand as C;

    match command {
        // --- generic bridge / observability -------------------------------
        C::GetState => AgentResponse::State(build_snapshot(ctx)),
        C::KeyInput(p) => handle_key_input(ctx.bridge, p),
        C::AxisInput(p) => handle_axis_input(ctx.bridge, p),
        C::PauseTime => {
            ctx.bridge.paused.set(true);
            AgentResponse::Ok
        }
        C::ResumeTime => {
            ctx.bridge.paused.set(false);
            ctx.bridge.requested_steps.set(0);
            AgentResponse::Ok
        }
        C::StepTime(p) => handle_step(ctx.bridge, p),
        C::FastForward(p) => {
            ctx.bridge.fast_forward.set(p.on);
            AgentResponse::Ok
        }
        C::Screenshot => dispatch_screenshot(ctx.bridge),
        C::LogTail(_) => AgentResponse::err(AgentError::internal(
            "log_tail must not be queued; served by transport",
        )),
        C::GetPerfMetrics => handle_get_perf_metrics(),

        // --- PAL3-specific gameplay surface --------------------------------
        C::TeleportPlayer(p) => handle_teleport(ctx, p),
        C::AdvanceDialog => {
            // Synthesise the same key the player taps to advance text
            // (Space is the PAL3 default; mirrors PAL4's behaviour).
            ctx.bridge.input_bridge.borrow().tap(Key::Space);
            AgentResponse::Ok
        }
        C::SaveSlot(p) => handle_save_slot(ctx, p),
        C::GetScriptGlobals(p) => handle_get_globals(ctx, p),
        C::SetStatusMenu(p) => handle_set_status_menu(ctx, p),

        // --- mode control: routed through the dispatcher in service.rs ----
        // `LoadSlot` (`/v1/load`) is unified with `EnterLoadGame`: PAL3 has
        // no in-place restore, so `Pal3Service::dispatch_mode_control`
        // rebuilds a fresh `AdventureDirector` from the slot for both. All
        // four are intercepted by `is_mode_control_command` before reaching
        // this per-command dispatcher; this arm is a defensive fallback so a
        // future refactor doesn't silently swallow them.
        C::EnterNewGame | C::EnterLoadGame(_) | C::LoadSlot(_) | C::ExitGame => {
            AgentResponse::err(AgentError::internal(
                "mode-control command leaked into the PAL3 per-command dispatcher",
            ))
        }

        // --- not yet implemented for PAL3 ---------------------------------
        C::ChooseDialog(_) => AgentResponse::err(AgentError::not_implemented(
            "PAL3 dialog choice buffering not yet implemented; the SCE dialog system reads the \
             current selection via SceProcContext::set_dlgsel directly",
        )),
        C::ChooseWorldMap(_) => AgentResponse::err(AgentError::not_implemented(
            "PAL3 has no world-map prompt analog to PAL4's giShowWorldMap",
        )),
        C::GetSceneTriggers => AgentResponse::err(AgentError::not_implemented(
            "PAL3 scene triggers are SCE proc entry points; enumeration deferred",
        )),
        C::GetSceneObjects => AgentResponse::err(AgentError::not_implemented(
            "PAL3 scene object enumeration deferred (no GOB research_function analog)",
        )),
        C::FireSceneTrigger(_) => AgentResponse::err(AgentError::not_implemented(
            "PAL3 fire_trigger deferred; will call SceVm::call_proc by name once mapping lands",
        )),
        C::InteractObject(_) => AgentResponse::err(AgentError::not_implemented(
            "PAL3 has no GOB Examine handler analog",
        )),
        C::ScriptEval(_) => AgentResponse::err(AgentError::not_implemented(
            "PAL3 script_eval is not supported (no AngelScript VM)",
        )),
        C::TraceStart(_) | C::TraceStop | C::TraceDrain(_) => {
            AgentResponse::err(AgentError::not_implemented(
                "PAL3 SceVm has no trace adapter yet; /v1/script/trace/* unavailable",
            ))
        }

        // AgentCommand is `#[non_exhaustive]` — surface any future
        // variants as `not_implemented` rather than panicking, so a
        // workspace upgrade to a newer agent_server doesn't bring
        // PAL3 down.
        other => AgentResponse::err(AgentError::not_implemented(format!(
            "PAL3 agent dispatcher does not yet implement {other:?}",
        ))),
    }
}

/// Build a [`StateSnapshot`] from whatever PAL3 state is reachable.
/// When no `AdventureDirector` is active (start menu / title) the
/// snapshot carries just the frame/dt/fps from the bridge.
pub fn build_snapshot(ctx: &Pal3DispatchCtx) -> StateSnapshot {
    let mut snap = StateSnapshot {
        frame: ctx.bridge.frame.get(),
        paused: ctx.bridge.paused.get(),
        fast_forward: ctx.bridge.fast_forward.get(),
        fps: ctx.bridge.fps_display.get(),
        dt: ctx.bridge.dt_display.get(),
        ..Default::default()
    };

    let Some(director) = ctx.director else {
        return snap;
    };

    let sce_vm = director.sce_vm();
    let global = sce_vm.global_state();
    let persistent = global.persistent_state();

    snap.scene = persistent.scene_name().unwrap_or_default();
    snap.block = persistent.sub_scene_name().unwrap_or_default();
    snap.leader = global.role_controlled();
    if let Some(pos) = director.controlled_role_position() {
        snap.leader_pos = [pos.x, pos.y, pos.z];
    }

    // PAL3's `adv_input_enabled` is set to `false` whenever the
    // engine is mid-cutscene, dialog, or movie. Treat its negation
    // as "script is running" for the agent contract — combined with
    // `is_running()` for the case where a proc is pending but input
    // is still nominally enabled (script kick-off frame).
    let script_running = !global.adv_input_enabled() || sce_vm.state().context().is_running();
    snap.script_running = script_running;
    snap.current_script_fn = sce_vm.state().context().current_proc_name();

    // PAL3 dialog/movie surfaces aren't readable from outside the
    // SceVm today; leave them at defaults rather than fabricate data.
    snap.dialog = DialogSnapshot::default();
    snap
}

fn handle_key_input(bridge: &Rc<AgentBridge>, params: KeyInputParams) -> AgentResponse {
    let Some(key) = Key::from_name(&params.key) else {
        return AgentResponse::err(AgentError::bad_request(format!(
            "unknown key name: {}",
            params.key
        )));
    };
    let synthetic = bridge.input_bridge.borrow();
    match params.action {
        KeyAction::Down => synthetic.press_down(key),
        KeyAction::Up => synthetic.release(key),
        KeyAction::Tap => synthetic.tap(key),
    }
    AgentResponse::Ok
}

fn handle_axis_input(bridge: &Rc<AgentBridge>, params: AxisInputParams) -> AgentResponse {
    let Some(axis) = Axis::from_name(&params.axis) else {
        return AgentResponse::err(AgentError::bad_request(format!(
            "unknown axis name: {}",
            params.axis
        )));
    };
    bridge.input_bridge.borrow().set_axis(axis, params.value);
    AgentResponse::Ok
}

fn handle_step(bridge: &Rc<AgentBridge>, params: StepTimeParams) -> AgentResponse {
    if !bridge.paused.get() {
        return AgentResponse::err(AgentError::conflict(
            "must pause time before requesting fixed-step frames",
        ));
    }
    if params.frames == 0 {
        return AgentResponse::Ok;
    }
    bridge
        .requested_steps
        .set(bridge.requested_steps.get().saturating_add(params.frames));
    bridge.requested_dt.set(params.dt.unwrap_or(0.0).max(0.0));
    AgentResponse::Ok
}

fn dispatch_screenshot(bridge: &Rc<AgentBridge>) -> AgentResponse {
    let engine = match bridge.rendering_engine.borrow().clone() {
        Some(e) => e,
        None => return AgentResponse::Screenshot(ScreenshotResponse::default()),
    };
    match engine.borrow_mut().capture_last_frame() {
        Some(frame) => AgentResponse::Screenshot(ScreenshotResponse {
            width: frame.width,
            height: frame.height,
            encoded: true,
            rgba: frame.rgba,
        }),
        None => AgentResponse::Screenshot(ScreenshotResponse::default()),
    }
}

fn handle_get_perf_metrics() -> AgentResponse {
    use agent_server::protocol::{PerfMetric, PerfMetricsResponse};

    let entries = radiance::perf::snapshot();
    let metrics = entries
        .into_iter()
        .map(|(name, snapshot)| match snapshot {
            radiance::perf::MetricSnapshot::Timing {
                calls,
                avg_ns,
                max_ns,
            } => PerfMetric::Timing {
                name: name.to_string(),
                calls,
                avg_ns,
                max_ns,
            },
            radiance::perf::MetricSnapshot::Counter { frame, total } => PerfMetric::Counter {
                name: name.to_string(),
                frame,
                total,
            },
            radiance::perf::MetricSnapshot::Gauge { last, max } => PerfMetric::Gauge {
                name: name.to_string(),
                last,
                max,
            },
        })
        .collect();
    AgentResponse::PerfMetrics(PerfMetricsResponse {
        enabled: radiance::perf::enabled(),
        metrics,
    })
}

fn handle_teleport(ctx: &Pal3DispatchCtx, params: TeleportParams) -> AgentResponse {
    let Some(director) = ctx.director else {
        return AgentResponse::err(AgentError::conflict(
            "no active adventure director — start a New Game before teleporting",
        ));
    };
    // PAL3 only models a single controlled role at any one time
    // (`GlobalState::role_controlled`); the `player` slot is
    // accepted but ignored beyond a basic bounds check so callers
    // get a clear error rather than silent acceptance of garbage.
    if params.player < 0 || params.player > 5 {
        return AgentResponse::err(AgentError::bad_request(format!(
            "PAL3 player slot must be in 0..=5, got {}",
            params.player
        )));
    }
    let pos = Vec3::new(params.pos[0], params.pos[1], params.pos[2]);
    if director.teleport_controlled_role(pos) {
        AgentResponse::Ok
    } else {
        AgentResponse::err(AgentError::conflict(
            "no scene loaded or controlled role cannot be resolved",
        ))
    }
}

fn handle_set_status_menu(ctx: &Pal3DispatchCtx, params: StatusMenuParams) -> AgentResponse {
    let Some(director) = ctx.director else {
        return AgentResponse::err(AgentError::conflict(
            "no active adventure director — start a New Game or load a slot before opening the \
             status menu",
        ));
    };
    // The renderer only paints (and so honors menu state) while the
    // player has control — i.e. no SCE proc is running. Toggling the
    // flag is always safe; it takes visible effect on the next idle
    // frame.
    director
        .sce_vm()
        .state()
        .status_renderer()
        .set_menu_open(params.open);
    AgentResponse::Ok
}

fn handle_save_slot(ctx: &Pal3DispatchCtx, params: SlotParams) -> AgentResponse {
    let Some(director) = ctx.director else {
        return AgentResponse::err(AgentError::conflict(
            "no active adventure director — cannot save without an in-progress playthrough",
        ));
    };
    if params.slot < 1 || params.slot > 4 {
        return AgentResponse::err(AgentError::bad_request(format!(
            "PAL3 save slot must be in 1..=4, got {}",
            params.slot
        )));
    }
    director
        .sce_vm()
        .global_state()
        .persistent_state()
        .save(params.slot);
    AgentResponse::Ok
}

fn handle_get_globals(ctx: &Pal3DispatchCtx, params: ScriptGlobalsParams) -> AgentResponse {
    let Some(director) = ctx.director else {
        return AgentResponse::ScriptGlobals(ScriptGlobalsResponse::default());
    };
    let sce_vm = director.sce_vm();
    let persistent = sce_vm.global_state().persistent_state();

    // PAL3 globals are sparse: walk a bounded probe to determine the
    // "effective length" (one past the highest written slot) and
    // then expose a dense window over [start, start+limit).
    let mut len: usize = 0;
    for i in 0..=GLOBALS_PROBE_MAX {
        if persistent.get_global(i).is_some() {
            len = (i as usize) + 1;
        }
    }
    let start = params.start;
    let limit = params.limit.unwrap_or(DEFAULT_GLOBALS_WINDOW);
    let end = start.saturating_add(limit).min(len.max(start));
    let mut globals = Vec::with_capacity(end.saturating_sub(start));
    for i in start..end {
        let v = persistent.get_global(i as i16).unwrap_or(0);
        // The wire format is `u32`, matching PAL4's bitcast convention.
        globals.push(v as u32);
    }
    AgentResponse::ScriptGlobals(ScriptGlobalsResponse {
        len,
        start,
        globals,
    })
}
