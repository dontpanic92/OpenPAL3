//! SWD5-family ([`GameType::SWD5`](crate::GameType::SWD5) /
//! [`SWDHC`](crate::GameType::SWDHC) /
//! [`SWDCF`](crate::GameType::SWDCF)) [`agent_server`] adapter.
//!
//! All three titles share one Lua 5.0 game layer
//! ([`OpenSWD5Director`](crate::openswd5::director::OpenSWD5Director) +
//! [`SWD5Context`]), so they also share this dispatcher; the only
//! per-game divergence lives in the asset loader.
//!
//! The shared, game-agnostic plumbing (queue, synthetic input,
//! pause/step cells, fast-forward, trace ring, rendering engine) lives
//! in [`crate::agent_common::AgentBridge`]; the generic command
//! handlers live in [`crate::agent_common::handlers`]. This module:
//!
//! * Builds [`StateSnapshot`]s from SWD5-family state ([`SWD5Context`]):
//!   map id, VM busy/movie flags, live dialog text and camera pose.
//! * Serves the gameplay endpoints this game layer can actually back —
//!   dialog advance, camera pose, map change (routed from
//!   `/v1/player/teleport`), Lua script globals and a narrow
//!   `script_eval` allow-list.
//! * Returns `NotImplemented` for endpoints with no counterpart in the
//!   game layer (save/load, menu control, dialog choice, scene
//!   triggers/objects, VM trace, world map).
//!
//! ## Why several endpoints stay unimplemented
//!
//! The SWD5-family game layer has **no role/actor entities**:
//! `chang_role_map`, `set_motion`, `set_walks` and
//! `set_role_face_motion` are empty stubs in
//! [`crate::openswd5::scripting`], and
//! [`Swd5Scene`](crate::openswd5::scene::Swd5Scene) holds only the map
//! DFF plus a camera. There is therefore no player position to report
//! or teleport to, and no NPC list to enumerate. Those endpoints are
//! blocked on game-layer role loading, not on the agent surface.
//!
//! The dispatcher is invoked from `Swd5Service::pump_agent`; it never
//! crosses the HTTP↔game thread boundary directly (everything goes
//! through the shared [`AgentCommandQueue`](agent_server::AgentCommandQueue)).

use std::cell::RefCell;
use std::rc::Rc;

use agent_server::protocol::{
    AgentCommand, AgentError, AgentResponse, DialogSnapshot, NamedGlobal, ScriptEvalParams,
    ScriptEvalResponse, ScriptGlobalsParams, ScriptGlobalsResponse, StateSnapshot, TeleportParams,
};
use radiance::input::Key;

use crate::agent_common::AgentBridge;
use crate::agent_common::handlers;
use crate::openswd5::director::OpenSWD5Director;
use crate::openswd5::scripting::SWD5Context;
use crate::scripting::lua50_32::LuaValue;

/// Stitched together once per command by `Swd5Service::pump_agent`.
///
/// `context` / `director` are `None` only in the brief window before
/// the first `OpenSWD5Director` is installed; once present they stay
/// for the rest of the launch (the SWD5 family has no start-menu /
/// title mode).
pub struct Swd5DispatchCtx<'a> {
    pub bridge: &'a Rc<AgentBridge>,
    pub context: Option<Rc<RefCell<SWD5Context>>>,
    /// Borrowed active director, used for Lua VM introspection
    /// (`/v1/script/globals`). Kept separate from `context` because the
    /// VM lives on the director, not the script context.
    pub director: Option<&'a OpenSWD5Director>,
}

/// Dispatch a single [`AgentCommand`] against the supplied SWD5-family
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

        // --- gameplay surface ---------------------------------------------
        C::SetCamera(p) => handle_set_camera(ctx, p),
        C::TeleportPlayer(p) => handle_teleport(ctx, p),
        C::GetScriptGlobals(p) => handle_script_globals(ctx, p),
        C::ScriptEval(p) => handle_script_eval(ctx, p),

        // --- blocked on absent game-layer features ------------------------
        C::SaveSlot(_) | C::EnterNewGame | C::EnterLoadGame(_) | C::LoadSlot(_) => {
            AgentResponse::err(AgentError::not_implemented(
                "SWD5 family has no save format support and boots straight into a \
                 single bootstrap script (no menu mode), so save/load and mode \
                 control have nothing to drive",
            ))
        }
        C::ExitGame => AgentResponse::err(AgentError::not_implemented(
            "SWD5 family has no menu mode to exit to",
        )),
        C::ChooseDialog(_) => AgentResponse::err(AgentError::not_implemented(
            "SWD5-family message boxes (storymsg / talkmsg) are free-form text \
             with no choice list; use /v1/dialog/advance instead",
        )),
        C::ChooseWorldMap(_) => AgentResponse::err(AgentError::not_implemented(
            "SWD5 family has no world-map prompt",
        )),
        C::GetSceneTriggers | C::FireSceneTrigger(_) => {
            AgentResponse::err(AgentError::not_implemented(
                "SWD5-family maps carry no EVF-equivalent trigger volumes; the Lua \
                 script drives all transitions directly",
            ))
        }
        C::GetSceneObjects | C::InteractObject(_) => {
            AgentResponse::err(AgentError::not_implemented(
                "SWD5 family has no role/actor entities (chang_role_map, set_motion \
                 and set_walks are stubs), so there is nothing to enumerate or \
                 interact with until game-layer role loading lands",
            ))
        }
        C::SetDebugCamera(_) => AgentResponse::err(AgentError::not_implemented(
            "SWD5 family has no free-fly debug-camera mode; place the camera \
             directly with /v1/camera/pose",
        )),
        C::TraceStart(_) | C::TraceStop | C::TraceDrain(_) => AgentResponse::err(
            AgentError::not_implemented("SWD5-family Lua VM has no trace adapter yet"),
        ),

        // AgentCommand is `#[non_exhaustive]` — surface any future
        // variants as `not_implemented` rather than panicking.
        other => AgentResponse::err(AgentError::not_implemented(format!(
            "SWD5-family agent dispatcher does not yet implement {other:?}",
        ))),
    }
}

/// `POST /v1/camera/pose` — place the camera at an absolute eye +
/// look-at target. Scripted `set_camera_src_pos` / `chang_camera_view`
/// calls can overwrite this on any later frame; pause time first for a
/// stable pose.
fn handle_set_camera(
    ctx: &Swd5DispatchCtx,
    params: agent_server::protocol::CameraPoseParams,
) -> AgentResponse {
    let context = match ctx.context.as_ref() {
        Some(c) => c,
        None => return AgentResponse::err(no_context_err()),
    };

    if context
        .borrow_mut()
        .agent_set_camera(params.eye, params.target)
    {
        AgentResponse::Ok
    } else {
        AgentResponse::err(AgentError::conflict(
            "no map is loaded yet; wait for /v1/state to report a non-zero scene",
        ))
    }
}

/// `POST /v1/player/teleport` — reinterpreted for the SWD5 family.
///
/// There is no player entity to move, so `pos[0]` is taken as the
/// target **map id** (the same value `/v1/state` reports as `scene`)
/// and routed through the engine's own `chang_map` path. `player` and
/// `pos[1..]` are ignored.
fn handle_teleport(ctx: &Swd5DispatchCtx, params: TeleportParams) -> AgentResponse {
    // Validate the payload before resolving the director, so a
    // malformed request is a 400 regardless of whether a map is
    // loaded yet.
    let map_id = params.pos[0];
    if !map_id.is_finite() || map_id < 0. {
        return AgentResponse::err(AgentError::bad_request(format!(
            "pos[0] must be a non-negative map id for the SWD5 family, got {map_id}"
        )));
    }

    let context = match ctx.context.as_ref() {
        Some(c) => c,
        None => return AgentResponse::err(no_context_err()),
    };

    match context.borrow_mut().agent_change_map(map_id as i32) {
        Ok(()) => AgentResponse::Ok,
        Err(e) => AgentResponse::err(AgentError::conflict(format!(
            "failed to load map {}: {e:?}",
            map_id as i32
        ))),
    }
}

/// `GET /v1/script/globals` — a windowed, name-sorted view of the Lua
/// global table with the stdlib and host functions filtered out.
///
/// The SWD5 family populates the `named` field rather than PAL3/PAL4's
/// flat `globals` index array, because its VM keys globals by name.
fn handle_script_globals(ctx: &Swd5DispatchCtx, params: ScriptGlobalsParams) -> AgentResponse {
    let director = match ctx.director {
        Some(d) => d,
        None => return AgentResponse::err(no_context_err()),
    };

    let all = director.script_globals();
    let len = all.len();
    let start = params.start.min(len);
    let end = match params.limit {
        Some(limit) => start.saturating_add(limit).min(len),
        None => len,
    };

    let named = all[start..end]
        .iter()
        .map(|(name, value)| NamedGlobal {
            name: name.clone(),
            value: lua_value_to_json(value),
        })
        .collect();

    AgentResponse::ScriptGlobals(ScriptGlobalsResponse {
        len,
        start,
        named,
        ..Default::default()
    })
}

/// `POST /v1/script/eval` — invoke one of the allow-listed SWD5 host
/// functions. Never re-enters the suspended Lua coroutine; see
/// [`SWD5Context::agent_eval`].
fn handle_script_eval(ctx: &Swd5DispatchCtx, params: ScriptEvalParams) -> AgentResponse {
    let context = match ctx.context.as_ref() {
        Some(c) => c,
        None => return AgentResponse::err(no_context_err()),
    };

    // Every scripted SWD5 builtin takes plain numbers.
    let mut args = Vec::with_capacity(params.args.len());
    for (i, arg) in params.args.iter().enumerate() {
        match arg.as_f64() {
            Some(v) => args.push(v),
            None => {
                return AgentResponse::err(AgentError::bad_request(format!(
                    "arg {i} must be a number for SWD5 script_eval, got {arg}"
                )));
            }
        }
    }

    match context.borrow_mut().agent_eval(&params.function, &args) {
        Ok(()) => AgentResponse::Script(ScriptEvalResponse {
            function: params.function,
            result: None,
        }),
        Err(msg) => AgentResponse::err(AgentError::bad_request(msg)),
    }
}

fn no_context_err() -> AgentError {
    AgentError::conflict("no SWD5 director is active yet; retry once /v1/state responds")
}

/// Marshal a [`LuaValue`] into the JSON carried by [`NamedGlobal`].
fn lua_value_to_json(value: &LuaValue) -> serde_json::Value {
    match value {
        LuaValue::Nil => serde_json::Value::Null,
        LuaValue::Bool(b) => serde_json::Value::Bool(*b),
        LuaValue::Number(n) => serde_json::Number::from_f64(*n)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        LuaValue::Str(s) => serde_json::Value::String(s.clone()),
        LuaValue::Other(tag) => serde_json::Value::String(format!("<{tag}>")),
    }
}

/// Build a [`StateSnapshot`] from whatever SWD5-family state is
/// reachable. Before the first director is installed the snapshot
/// carries just the frame/dt/fps/pause/fast-forward fields from the
/// bridge.
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

        // Live message-box text. `script_running` is the VM-level
        // truth; `dialog.open` is the "waiting on the player" signal —
        // a driver should tap /v1/dialog/advance while it is `true`.
        snap.dialog = dialog_snapshot(context.dialog_text());

        if let Some((eye, target)) = context.camera_pose() {
            snap.camera_eye = eye;
            snap.camera_target = target;
        }
    }

    snap
}

/// Map the SWD5-family message-box state onto a [`DialogSnapshot`].
///
/// SWD5 has no left/right avatar concept, so `avatar` carries the
/// `talkmsg` speaker name (empty for a plain `storymsg`), and
/// `choices` stays empty — the game has no select-dialog.
fn dialog_snapshot(text: Option<(String, String)>) -> DialogSnapshot {
    match text {
        Some((speaker, text)) => DialogSnapshot {
            open: true,
            text,
            avatar: speaker,
            choices: Vec::new(),
        },
        None => DialogSnapshot::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openswd5::scripting::eval_rejection_message;
    use agent_server::protocol::CameraPoseParams;
    use radiance::input::{Axis, AxisState, InputEngine, KeyState, SyntheticInputBridge};

    /// Minimal inner engine so a `SyntheticInputBridge` (and therefore
    /// an `AgentBridge`) can be built without Vulkan or a window.
    struct StubEngine;
    impl InputEngine for StubEngine {
        fn get_key_state(&self, _key: Key) -> KeyState {
            KeyState::default()
        }
        fn get_axis_state(&self, _axis: Axis) -> AxisState {
            AxisState::default()
        }
    }

    /// A bridge with no director installed — the state every command
    /// sees during the window before the first `OpenSWD5Director`.
    fn bridge() -> Rc<AgentBridge> {
        let input = Rc::new(RefCell::new(SyntheticInputBridge::new(
            Rc::new(RefCell::new(StubEngine)) as Rc<RefCell<dyn InputEngine>>,
        )));
        Rc::new(AgentBridge::new(input))
    }

    fn ctx(bridge: &Rc<AgentBridge>) -> Swd5DispatchCtx<'_> {
        Swd5DispatchCtx {
            bridge,
            context: None,
            director: None,
        }
    }

    #[test]
    fn snapshot_without_context_is_all_defaults() {
        let bridge = bridge();
        let snap = build_snapshot(&ctx(&bridge));

        assert_eq!(snap.scene, "");
        assert!(!snap.script_running);
        assert!(!snap.movie_playing);
        assert!(!snap.dialog.open);
        assert_eq!(snap.dialog.text, "");
        assert_eq!(snap.camera_eye, [0., 0., 0.]);
        assert_eq!(snap.camera_target, [0., 0., 0.]);
    }

    #[test]
    fn snapshot_reflects_bridge_pause_and_fast_forward() {
        let bridge = bridge();
        bridge.paused.set(true);
        bridge.fast_forward.set(true);

        let snap = build_snapshot(&ctx(&bridge));
        assert!(snap.paused);
        assert!(snap.fast_forward);
    }

    #[test]
    fn dialog_snapshot_maps_talkmsg_speaker_to_avatar() {
        let snap = dialog_snapshot(Some(("景天".into(), "你是谁？".into())));
        assert!(snap.open);
        assert_eq!(snap.avatar, "景天");
        assert_eq!(snap.text, "你是谁？");
        assert!(snap.choices.is_empty(), "SWD5 has no select-dialog");
    }

    #[test]
    fn dialog_snapshot_storymsg_has_no_speaker() {
        let snap = dialog_snapshot(Some((String::new(), "旁白".into())));
        assert!(snap.open);
        assert_eq!(snap.avatar, "");
        assert_eq!(snap.text, "旁白");
    }

    #[test]
    fn dialog_snapshot_closed_when_no_message() {
        let snap = dialog_snapshot(None);
        assert!(!snap.open);
        assert_eq!(snap.text, "");
    }

    #[test]
    fn eval_allow_list_rejects_unknown_function() {
        let msg = eval_rejection_message("giAddMoney");
        assert!(msg.contains("giAddMoney"));
        assert!(msg.contains("chang_map"), "message lists the allow-list");
        assert!(!SWD5Context::EVAL_ALLOW_LIST.contains(&"giAddMoney"));
    }

    #[test]
    fn eval_allow_list_contains_only_safe_host_functions() {
        // Everything on the list must be a pure state mutation; in
        // particular nothing that resumes the Lua coroutine.
        for name in SWD5Context::EVAL_ALLOW_LIST {
            assert!(!name.starts_with("sleep"), "{name} would block the VM");
            assert_ne!(*name, "anykey", "anykey reads input, not state");
        }
    }

    #[test]
    fn lua_values_marshal_to_json() {
        assert_eq!(lua_value_to_json(&LuaValue::Nil), serde_json::Value::Null);
        assert_eq!(
            lua_value_to_json(&LuaValue::Bool(true)),
            serde_json::json!(true)
        );
        assert_eq!(
            lua_value_to_json(&LuaValue::Number(3.5)),
            serde_json::json!(3.5)
        );
        assert_eq!(
            lua_value_to_json(&LuaValue::Str("阿奴".into())),
            serde_json::json!("阿奴")
        );
        assert_eq!(
            lua_value_to_json(&LuaValue::Other("function")),
            serde_json::json!("<function>")
        );
    }

    #[test]
    fn non_finite_lua_number_degrades_to_null() {
        assert_eq!(
            lua_value_to_json(&LuaValue::Number(f64::NAN)),
            serde_json::Value::Null
        );
    }

    #[test]
    fn teleport_rejects_negative_map_id() {
        let bridge = bridge();
        let resp = handle_teleport(
            &ctx(&bridge),
            TeleportParams {
                player: 0,
                pos: [-1., 0., 0.],
            },
        );

        match resp {
            AgentResponse::Error(e) => {
                assert_eq!(e.kind, agent_server::protocol::AgentErrorKind::BadRequest)
            }
            other => panic!("expected bad_request, got {other:?}"),
        }
    }

    #[test]
    fn gameplay_commands_conflict_before_a_director_exists() {
        let bridge = bridge();

        // A valid map id, but no director is installed yet.
        let resp = handle_teleport(
            &ctx(&bridge),
            TeleportParams {
                player: 0,
                pos: [3., 0., 0.],
            },
        );
        assert!(matches!(resp, AgentResponse::Error(_)));

        let resp = handle_set_camera(
            &ctx(&bridge),
            CameraPoseParams {
                eye: [1., 2., 3.],
                target: [0., 0., 0.],
            },
        );
        assert!(matches!(resp, AgentResponse::Error(_)));

        let resp = handle_script_globals(&ctx(&bridge), ScriptGlobalsParams::default());
        assert!(matches!(resp, AgentResponse::Error(_)));
    }

    #[test]
    fn generic_commands_work_without_a_director() {
        let bridge = bridge();

        assert!(matches!(
            dispatch_swd5_command(&ctx(&bridge), AgentCommand::PauseTime),
            AgentResponse::Ok
        ));
        assert!(bridge.paused.get());

        assert!(matches!(
            dispatch_swd5_command(&ctx(&bridge), AgentCommand::AdvanceDialog),
            AgentResponse::Ok
        ));
        assert!(
            bridge
                .input_bridge
                .borrow()
                .get_key_state(Key::Space)
                .pressed(),
            "advance_dialog taps Space"
        );
    }

    #[test]
    fn role_dependent_commands_report_the_real_blocker() {
        let bridge = bridge();
        let resp = dispatch_swd5_command(&ctx(&bridge), AgentCommand::GetSceneObjects);

        match resp {
            AgentResponse::Error(e) => {
                assert_eq!(
                    e.kind,
                    agent_server::protocol::AgentErrorKind::NotImplemented
                );
                assert!(
                    e.message.contains("role"),
                    "message should name the missing feature: {}",
                    e.message
                );
            }
            other => panic!("expected not_implemented, got {other:?}"),
        }
    }
}
