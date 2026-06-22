//! PAL5-specific [`agent_server`] adapter (hosted in `yaobow_lib`,
//! since the PAL5 story runtime lives here rather than in `shared`).
//!
//! The shared, game-agnostic plumbing (queue, synthetic input,
//! pause/step cells, fast-forward, rendering engine) lives in
//! [`shared::agent_common::AgentBridge`]; the generic command handlers
//! live in [`shared::agent_common::handlers`]. This module:
//!
//! * Builds [`StateSnapshot`]s from PAL5 state ([`Pal5ScriptContext`]).
//! * Dispatches [`AgentCommand`]s to the generic bridge surface and
//!   returns `NotImplemented` for the per-game gameplay endpoints PAL5
//!   has no clean mapping for yet.
//!
//! The dispatcher is invoked from `Pal5Service::pump_agent` (the single
//! per-frame drainer); it never crosses the HTTP↔game thread boundary
//! directly (everything goes through the shared command queue).

use std::cell::RefCell;
use std::rc::Rc;

use agent_server::protocol::{
    AgentCommand, AgentError, AgentResponse, DialogSnapshot, StateSnapshot,
};
use radiance::input::Key;
use shared::agent_common::AgentBridge;
use shared::agent_common::handlers;

use super::context::Pal5ScriptContext;

/// Stitched together once per command by `Pal5Service::pump_agent`.
///
/// The `context` is `None` only in the brief window before the first
/// `Pal5StoryDirector` is installed; once present it stays for the rest
/// of the launch (PAL5 has no start-menu / title mode).
pub struct Pal5DispatchCtx<'a> {
    pub bridge: &'a Rc<AgentBridge>,
    pub context: Option<Rc<RefCell<Pal5ScriptContext>>>,
}

/// Dispatch a single [`AgentCommand`] against the supplied PAL5
/// context. The reply is wired straight back to the HTTP client by
/// the surrounding [`pump_agent`] loop.
pub fn dispatch_pal5_command(ctx: &Pal5DispatchCtx, command: AgentCommand) -> AgentResponse {
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

        // PAL5 advances Wait / dialog on any key (Space / Escape /
        // GamePadSouth); synthesise the Space tap the player presses.
        C::AdvanceDialog => {
            ctx.bridge.input_bridge.borrow().tap(Key::Space);
            AgentResponse::Ok
        }

        // --- not yet implemented for PAL5 ---------------------------------
        C::TeleportPlayer(_) => AgentResponse::err(AgentError::not_implemented(
            "PAL5 has no controlled-role teleport surface yet",
        )),
        C::SaveSlot(_) | C::EnterNewGame | C::EnterLoadGame(_) | C::LoadSlot(_) => {
            AgentResponse::err(AgentError::not_implemented(
                "PAL5 save/load + mode control are not implemented (single bootstrap script)",
            ))
        }
        C::ExitGame => AgentResponse::err(AgentError::not_implemented(
            "PAL5 exit-to-menu is not implemented (no menu mode)",
        )),
        C::ChooseDialog(_) => AgentResponse::err(AgentError::not_implemented(
            "PAL5 dialog choice buffering not yet implemented",
        )),
        C::ChooseWorldMap(_) => {
            AgentResponse::err(AgentError::not_implemented("PAL5 has no world-map prompt"))
        }
        C::GetSceneTriggers | C::FireSceneTrigger(_) => AgentResponse::err(
            AgentError::not_implemented("PAL5 scene-trigger enumeration deferred"),
        ),
        C::GetSceneObjects | C::InteractObject(_) => AgentResponse::err(
            AgentError::not_implemented("PAL5 scene-object enumeration deferred"),
        ),
        C::GetScriptGlobals(_) => AgentResponse::err(AgentError::not_implemented(
            "PAL5 script-globals exposure deferred (Lua flag table)",
        )),
        C::ScriptEval(_) => AgentResponse::err(AgentError::not_implemented(
            "PAL5 script_eval is not supported",
        )),
        C::TraceStart(_) | C::TraceStop | C::TraceDrain(_) => AgentResponse::err(
            AgentError::not_implemented("PAL5 Lua VM has no trace adapter yet"),
        ),

        // AgentCommand is `#[non_exhaustive]` — surface any future
        // variants as `not_implemented` rather than panicking.
        other => AgentResponse::err(AgentError::not_implemented(format!(
            "PAL5 agent dispatcher does not yet implement {other:?}",
        ))),
    }
}

/// Build a [`StateSnapshot`] from whatever PAL5 state is reachable.
pub fn build_snapshot(ctx: &Pal5DispatchCtx) -> StateSnapshot {
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
        snap.scene = context.current_scene_name();
        if let Some(pos) = context.leader_position() {
            snap.leader_pos = pos;
        }
        // The VM is "running" whenever it isn't parked in a `Wait`.
        snap.script_running = !context.is_sleeping();
    }

    // PAL5 dialog text isn't structured for the agent yet; leave the
    // dialog snapshot at its default rather than fabricate.
    snap.dialog = DialogSnapshot::default();
    snap
}
