//! SWD5-family [`agent_server`] adapter.
//!
//! The shared, game-agnostic plumbing (queue, synthetic input,
//! pause/step cells, fast-forward, trace ring, rendering engine) lives
//! in [`crate::agent_common::AgentBridge`]; the generic command
//! handlers live in [`crate::agent_common::handlers`]. This module:
//!
//! * Builds [`StateSnapshot`]s from SWD5 state ([`SWD5Context`]).
//! * Dispatches [`AgentCommand`]s to the generic bridge surface, and
//!   returns `NotImplemented` for the per-game gameplay endpoints SWD5
//!   has no clean mapping for yet (save/load, teleport, dialog choice,
//!   scene triggers/objects, script globals/eval/trace, world map).
//!
//! The dispatcher is invoked from `Swd5Service::pump_agent`; it never
//! crosses the HTTP↔game thread boundary directly (everything goes
//! through the shared [`AgentCommandQueue`](agent_server::AgentCommandQueue)).

use std::cell::RefCell;
use std::rc::Rc;

use agent_server::protocol::{
    AgentCommand, AgentError, AgentResponse, DialogSnapshot, StateSnapshot,
};
use radiance::input::Key;

use crate::agent_common::AgentBridge;
use crate::agent_common::handlers;
use crate::openswd5::scripting::SWD5Context;

/// Stitched together once per command by `Swd5Service::pump_agent`.
///
/// The `context` is `None` only in the brief window before the first
/// `OpenSWD5Director` is installed; once present it stays for the rest
/// of the launch (SWD5 has no start-menu / title mode).
pub struct Swd5DispatchCtx<'a> {
    pub bridge: &'a Rc<AgentBridge>,
    pub context: Option<Rc<RefCell<SWD5Context>>>,
}

/// Dispatch a single [`AgentCommand`] against the supplied SWD5
/// context. The reply is wired straight back to the HTTP client by
/// the surrounding `pump_agent` loop.
pub fn dispatch_swd5_command(ctx: &Swd5DispatchCtx, command: AgentCommand) -> AgentResponse {
    use AgentCommand as C;

    match command {
        // --- generic bridge / observability -------------------------------
        C::GetState => AgentResponse::State(build_snapshot(ctx)),
        C::KeyInput(p) => handlers::handle_key_input(ctx.bridge, p),
        C::AxisInput(p) => handlers::handle_axis_input(ctx.bridge, p),
        C::PauseTime => {
            ctx.bridge.paused.set(true);
            AgentResponse::Ok
        }
        C::ResumeTime => {
            ctx.bridge.paused.set(false);
            ctx.bridge.requested_steps.set(0);
            AgentResponse::Ok
        }
        C::StepTime(p) => handlers::handle_step(ctx.bridge, p),
        C::FastForward(p) => {
            ctx.bridge.fast_forward.set(p.on);
            AgentResponse::Ok
        }
        C::Screenshot => handlers::handle_screenshot(ctx.bridge),
        C::LogTail(_) => AgentResponse::err(AgentError::internal(
            "log_tail must not be queued; served by transport",
        )),
        C::GetPerfMetrics => handlers::handle_perf_metrics(),

        // SWD5 advances story/talk message boxes on any key (Space /
        // Escape / GamePadSouth); synthesise the Space tap the player
        // would press.
        C::AdvanceDialog => {
            ctx.bridge.input_bridge.borrow().tap(Key::Space);
            AgentResponse::Ok
        }

        // --- not yet implemented for SWD5 ---------------------------------
        C::TeleportPlayer(_) => AgentResponse::err(AgentError::not_implemented(
            "SWD5 has no controlled-role teleport surface yet",
        )),
        C::SaveSlot(_) | C::EnterNewGame | C::EnterLoadGame(_) | C::LoadSlot(_) => {
            AgentResponse::err(AgentError::not_implemented(
                "SWD5 save/load + mode control are not implemented (single bootstrap script)",
            ))
        }
        C::ExitGame => AgentResponse::err(AgentError::not_implemented(
            "SWD5 exit-to-menu is not implemented (no menu mode)",
        )),
        C::ChooseDialog(_) => AgentResponse::err(AgentError::not_implemented(
            "SWD5 dialog choice buffering not yet implemented",
        )),
        C::ChooseWorldMap(_) => AgentResponse::err(AgentError::not_implemented(
            "SWD5 has no world-map prompt",
        )),
        C::GetSceneTriggers | C::FireSceneTrigger(_) => AgentResponse::err(
            AgentError::not_implemented("SWD5 scene-trigger enumeration deferred"),
        ),
        C::GetSceneObjects | C::InteractObject(_) => AgentResponse::err(
            AgentError::not_implemented("SWD5 scene-object enumeration deferred"),
        ),
        C::GetScriptGlobals(_) => AgentResponse::err(AgentError::not_implemented(
            "SWD5 script-globals exposure deferred",
        )),
        C::ScriptEval(_) => AgentResponse::err(AgentError::not_implemented(
            "SWD5 script_eval is not supported",
        )),
        C::TraceStart(_) | C::TraceStop | C::TraceDrain(_) => AgentResponse::err(
            AgentError::not_implemented("SWD5 Lua VM has no trace adapter yet"),
        ),

        // AgentCommand is `#[non_exhaustive]` — surface any future
        // variants as `not_implemented` rather than panicking.
        other => AgentResponse::err(AgentError::not_implemented(format!(
            "SWD5 agent dispatcher does not yet implement {other:?}",
        ))),
    }
}

/// Build a [`StateSnapshot`] from whatever SWD5 state is reachable.
/// Before the first director is installed the snapshot carries just
/// the frame/dt/fps/pause/fast-forward fields from the bridge.
pub fn build_snapshot(ctx: &Swd5DispatchCtx) -> StateSnapshot {
    let mut snap = StateSnapshot {
        frame: ctx.bridge.frame.get(),
        paused: ctx.bridge.paused.get(),
        fast_forward: ctx.bridge.fast_forward.get(),
        fps: ctx.bridge.fps_display.get(),
        dt: ctx.bridge.dt_display.get(),
        ..Default::default()
    };

    if let Some(context) = ctx.context.as_ref() {
        let context = context.borrow();
        // SWD5 scenes are numeric map ids; expose the current one as
        // the `scene` field so callers can correlate map changes.
        snap.scene = context.current_map_id().to_string();
        // The VM is "running" whenever it isn't parked in a `sleep`.
        snap.script_running = !context.is_sleeping();
        snap.movie_playing = context.is_movie_playing();
    }

    // SWD5 dialog text isn't structured (free-form story/talk boxes);
    // leave the dialog snapshot at its default rather than fabricate.
    snap.dialog = DialogSnapshot::default();
    snap
}
