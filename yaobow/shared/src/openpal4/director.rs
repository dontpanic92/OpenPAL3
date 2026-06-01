use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use agent_server::protocol::{
    AgentCommand, AgentError, AgentResponse, AxisInputParams, DialogSnapshot, FastForwardParams,
    KeyAction, KeyInputParams, PartyMember, ScreenshotResponse, ScriptEvalParams, StateSnapshot,
    StepTimeParams, TeleportParams,
};
use crosscom::ComRc;
use radiance::comdef::{IImmediateDirectorImpl, IUiHost};
use radiance::math::Vec3;
use radiance::{
    audio::AudioEngine,
    comdef::{IDirectorImpl, ISceneManager},
    input::{Axis, InputEngine, Key},
    radiance::{TaskManager, UiManager},
    rendering::ComponentFactory,
    scene::CoreScene,
    utils::free_view::FreeViewController,
};

use crate::scripting::angelscript::ScriptVm;

use super::{
    agent::Pal4AgentBridge,
    app_context::{DialogAvatarSide, Pal4AppContext},
    asset_loader::AssetLoader,
    comdef::pal4_debug::{IPal4DebugContext, IPal4DebugOverlay},
    pal4_debug::Pal4DebugState,
    scripting::create_script_vm,
    states::persistent_state::Pal4PersistentState,
};

/// Bundle of script-side handles the director uses to drive the
/// protosept debug overlay each frame. Set by the application loader
/// after PAL4 debug session creation + the bootstrap script have
/// run; the director only reads from this bundle inside `render_im`.
pub struct Pal4DebugBundle {
    pub overlay: ComRc<IPal4DebugOverlay>,
    pub overlay_ctx: ComRc<IPal4DebugContext>,
    pub debug_state: Rc<Pal4DebugState>,
}

pub struct OpenPAL4Director {
    vm: RefCell<ScriptVm<Pal4AppContext>>,
    #[allow(dead_code)]
    control: FreeViewController,

    // Debug overlay state. `debug` is `None` when the protosept runtime
    // failed to bootstrap (e.g. on a build target where ScriptHost is
    // unavailable) — `render_im` no-ops cleanly in that case.
    debug: RefCell<Option<Pal4DebugBundle>>,
    debug_visible: Cell<bool>,
    debug_prev_tilde: Cell<bool>,
    fps_smoothed: Cell<f32>,

    // Perf-metric display throttle: the FPS/dt readouts are republished
    // only every `DEBUG_METRIC_INTERVAL_SEC` so they stay legible. The
    // EMA above keeps updating every frame; these only gate the values
    // copied into the overlay snapshot.
    debug_metric_accum: Cell<f32>,
    fps_display: Cell<f32>,
    dt_display: Cell<f32>,

    /// Agent-surface bridge. `None` when the binary did not pass
    /// `--agent-port`. When `Some`, the director drains the queue
    /// once per `update`, honours pause / step requests, ticks the
    /// synthetic input overlay, and mirrors fps / dt into the bridge
    /// for `GET /v1/state`.
    agent: RefCell<Option<Rc<Pal4AgentBridge>>>,
}

/// How often the debug overlay republishes the FPS / frame-time
/// readouts. Tuned so the numbers are readable rather than strobing.
const DEBUG_METRIC_INTERVAL_SEC: f32 = 0.5;

ComObject_OpenPAL4Director!(super::OpenPAL4Director);

impl OpenPAL4Director {
    pub fn new(
        component_factory: Rc<dyn ComponentFactory>,
        loader: Rc<AssetLoader>,
        scene_manager: ComRc<ISceneManager>,
        ui: Rc<UiManager>,
        input: Rc<RefCell<dyn InputEngine>>,
        audio: Rc<dyn AudioEngine>,
        task_manager: Rc<TaskManager>,
    ) -> Self {
        let app_context = Pal4AppContext::new(
            component_factory,
            loader,
            scene_manager,
            ui,
            input.clone(),
            audio,
            task_manager,
        );
        Self {
            vm: RefCell::new(create_script_vm(app_context)),
            control: FreeViewController::new(input),
            debug: RefCell::new(None),
            debug_visible: Cell::new(false),
            debug_prev_tilde: Cell::new(false),
            fps_smoothed: Cell::new(0.0),
            debug_metric_accum: Cell::new(0.0),
            fps_display: Cell::new(0.0),
            dt_display: Cell::new(0.0),
            agent: RefCell::new(None),
        }
    }

    /// Install the agent-server bridge. Idempotent — passing a new
    /// bridge replaces the previous one. Must be called *before* the
    /// engine starts pumping `update` for the per-frame drain to
    /// pick up any commands the HTTP listener has queued.
    pub fn set_agent_bridge(&self, bridge: Rc<Pal4AgentBridge>) {
        *self.agent.borrow_mut() = Some(bridge);
    }

    /// Borrow the installed agent bridge for read-only inspection.
    pub fn agent_bridge(&self) -> Option<Rc<Pal4AgentBridge>> {
        self.agent.borrow().clone()
    }

    /// Install the protosept-authored debug overlay. Idempotent —
    /// passing a new bundle replaces the previous one. Safe to call
    /// after construction but before the engine starts pumping
    /// `render_im`.
    pub fn set_debug_bundle(&self, bundle: Pal4DebugBundle) {
        *self.debug.borrow_mut() = Some(bundle);
    }

    /// Install the scripted `IPal4ActorController` factory on the
    /// underlying `Pal4AppContext`. Must be called before the first
    /// `load_scene` for the controllers to attach.
    pub fn set_actor_controller_factory(
        &self,
        factory: Rc<dyn super::scene::Pal4ActorControllerFactory>,
    ) {
        self.vm
            .borrow_mut()
            .app_context_mut()
            .set_actor_controller_factory(factory);
    }

    fn refresh_debug_snapshot(&self, delta_sec: f32) {
        let bundle_ref = self.debug.borrow();
        let Some(bundle) = bundle_ref.as_ref() else {
            return;
        };

        // Exponential moving average so the FPS readout doesn't strobe
        // on jittery frames. alpha tuned to roughly half a second.
        let inst_fps = if delta_sec > 1e-4 {
            1.0 / delta_sec
        } else {
            0.0
        };
        let prev = self.fps_smoothed.get();
        let fps = if prev <= 0.0 {
            inst_fps
        } else {
            prev * 0.9 + inst_fps * 0.1
        };
        self.fps_smoothed.set(fps);

        // Throttle the *displayed* FPS / frame-time. The EMA above runs
        // every frame for accuracy, but we only republish the readout
        // every `DEBUG_METRIC_INTERVAL_SEC` so the numbers are legible.
        // Seed on the first frame so the overlay isn't blank until the
        // first interval elapses.
        if self.fps_display.get() <= 0.0 {
            self.fps_display.set(fps);
            self.dt_display.set(delta_sec);
        }
        let accum = self.debug_metric_accum.get() + delta_sec;
        if accum >= DEBUG_METRIC_INTERVAL_SEC {
            self.fps_display.set(fps);
            self.dt_display.set(delta_sec);
            self.debug_metric_accum
                .set(accum - DEBUG_METRIC_INTERVAL_SEC);
        } else {
            self.debug_metric_accum.set(accum);
        }

        // Fan the script-side overlay toggles out to the live scene
        // before we snapshot, so the next overlay frame sees the
        // result of any button press from the previous frame. Done
        // per-frame so scene reloads pick up the current state with
        // no extra wiring.
        let bsp_visible = bundle.debug_state.bsp_visible();
        let nav_mesh_visible = bundle.debug_state.nav_mesh_visible();
        let fast_forward = bundle.debug_state.fast_forward();

        let mut vm = self.vm.borrow_mut();
        let app = vm.app_context_mut();
        app.set_bsp_visible(bsp_visible);
        app.set_nav_mesh_visible(nav_mesh_visible);
        app.set_fast_forward(fast_forward);

        let pos = app.leader_pos();
        bundle
            .debug_state
            .set_snapshot(super::pal4_debug::Pal4DebugSnapshot {
                scene_name: app.scene_name().to_string(),
                block_name: app.block_name().to_string(),
                leader_index: app.leader() as i32,
                leader_pos: [pos.x, pos.y, pos.z],
                delta_time: self.dt_display.get(),
                fps: self.fps_display.get(),
            });
    }

    fn poll_tilde(&self) -> bool {
        let vm = self.vm.borrow();
        let input = vm.app_context.input.borrow();
        let pressed = input.get_key_state(Key::Tilde).pressed();
        let prev = self.debug_prev_tilde.get();
        self.debug_prev_tilde.set(pressed);
        pressed && !prev
    }

    /// Persist the current game state to `slot` as JSON. Snapshots the
    /// shared angelscript globals (story-plot flags) plus the live
    /// scene/block/leader/position so a later load resumes at the same
    /// point. Negative slots are ignored by `Pal4PersistentState::save`.
    pub fn save_state(&self, slot: i32) {
        let mut vm = self.vm.borrow_mut();
        let globals = vm.g.borrow().globals_snapshot();

        let app = vm.app_context_mut();
        let pos = app.leader_pos();
        let scene = app.scene_name().to_string();
        let block = app.block_name().to_string();
        let leader = app.leader();

        let state = app.persistent_state_mut();
        state.set_script_globals(globals);
        state.set_scene(scene, block);
        state.set_leader(leader);
        state.set_position(Some(pos));
        state.save(slot);
    }

    /// Restore game state from `slot`, reloading the saved scene and
    /// repositioning the leader. No-ops (with a log) when the slot file
    /// is missing or malformed.
    pub fn load_state(&self, slot: i32) {
        let app_name = self
            .vm
            .borrow()
            .app_context()
            .persistent_state()
            .app_name()
            .to_string();

        let state = match Pal4PersistentState::load(&app_name, slot) {
            Ok(state) => state,
            Err(e) => {
                log::error!("Cannot load save slot {}: {}", slot, e);
                return;
            }
        };

        let scene = state.scene_name().to_string();
        let block = state.block_name().to_string();
        let leader = state.leader();
        let pos = state.position();
        let globals = state.script_globals().to_vec();

        let mut vm = self.vm.borrow_mut();
        vm.g.borrow_mut().restore_globals(&globals);

        let app = vm.app_context_mut();
        app.set_persistent_state(state);

        if !scene.is_empty() {
            app.load_scene(&scene, &block);
            app.set_leader(leader as i32);
            if let Some(pos) = pos {
                app.set_player_pos(leader as i32, &pos);
            }
        }

        log::info!("Game loaded from slot {}", slot);
    }

    /// Slot-based save/load via number-key hotkeys (mirrors OpenPAL3's
    /// `test_save`): Num1-Num4 load slots 1-4, Num5-Num8 save slots
    /// 1-4. Each `pressed()` is edge-triggered so a held key fires once.
    fn poll_save_load_hotkeys(&self) {
        let (save_slot, load_slot) = {
            let vm = self.vm.borrow();
            let input = vm.app_context.input.borrow();
            let save_slot = if input.get_key_state(Key::Num5).pressed() {
                1
            } else if input.get_key_state(Key::Num6).pressed() {
                2
            } else if input.get_key_state(Key::Num7).pressed() {
                3
            } else if input.get_key_state(Key::Num8).pressed() {
                4
            } else {
                -1
            };
            let load_slot = if input.get_key_state(Key::Num1).pressed() {
                1
            } else if input.get_key_state(Key::Num2).pressed() {
                2
            } else if input.get_key_state(Key::Num3).pressed() {
                3
            } else if input.get_key_state(Key::Num4).pressed() {
                4
            } else {
                -1
            };
            (save_slot, load_slot)
        };

        if save_slot >= 0 {
            self.save_state(save_slot);
        } else if load_slot >= 0 {
            self.load_state(load_slot);
        }
    }
}

impl IDirectorImpl for OpenPAL4Director {
    fn activate(&self) {
        self.vm
            .borrow()
            .app_context
            .scene_manager
            .push_scene(CoreScene::create());
    }

    fn update(&self, delta_sec: f32) -> Option<crosscom::ComRc<radiance::comdef::IDirector>> {
        // 1. Drain any agent commands queued by the HTTP listener.
        //    Done first so `pause` / `step` / `input` requests take
        //    effect on this same tick.
        self.drain_agent_commands();

        // 2. Decide whether this real frame actually advances the
        //    simulation. Pause + zero pending steps = full freeze
        //    (input bridge still ticks so a held key set just before
        //    pause stays sensible; that's handled below).
        let (advance, effective_dt) = self.compute_effective_dt(delta_sec);

        if !advance {
            self.end_agent_frame(delta_sec);
            return None;
        }

        self.vm.borrow_mut().app_context_mut().update(effective_dt);

        self.poll_save_load_hotkeys();

        if self.vm.borrow().context.is_none() {
            let function = self
                .vm
                .borrow_mut()
                .app_context_mut()
                .event_triggered(effective_dt);
            if let Some(function) = function {
                let module = self.vm.borrow().app_context.scene.module.clone().unwrap();
                self.vm
                    .borrow_mut()
                    .set_function_by_name2(module, &function);
            }
        }

        self.vm.borrow_mut().execute(effective_dt);

        /*if !self.vm.borrow().app_context().player_locked {
            self.control
                .update(scene_manager.scene().unwrap(), delta_sec)
        }*/

        self.end_agent_frame(effective_dt);

        None
    }

    fn deactivate(&self) {}
}

impl IImmediateDirectorImpl for OpenPAL4Director {
    fn render_im(&self, ui: ComRc<IUiHost>, dt: f32) {
        // Tilde edge-detect → toggle visibility. Done in Rust (rather
        // than in script) so the script overlay stays oblivious to the
        // input engine: it only needs to know "render me when called".
        if self.poll_tilde() {
            self.debug_visible.set(!self.debug_visible.get());
        }

        if !self.debug_visible.get() {
            return;
        }

        self.refresh_debug_snapshot(dt);

        // Clone the COM handles out of the RefCell first so we drop the
        // borrow before re-entering script land (the script may call
        // back into host services that touch the director).
        let (overlay, ctx) = {
            let bundle_ref = self.debug.borrow();
            let Some(bundle) = bundle_ref.as_ref() else {
                return;
            };
            (bundle.overlay.clone(), bundle.overlay_ctx.clone())
        };
        overlay.render(ui, dt, ctx);
    }
}

// === Agent-server integration ============================================
//
// Everything below this line is only exercised when `--agent-port` is
// set at boot. The plain windowed flow ignores the bridge (it stays
// `None`) and runs exactly as before.

impl OpenPAL4Director {
    /// Drain every command queued by the HTTP listener since the last
    /// frame, dispatching each into `dispatch_agent_command` and
    /// shipping the response back. Safe to call when no bridge is
    /// installed — returns immediately.
    fn drain_agent_commands(&self) {
        let bridge = match self.agent.borrow().clone() {
            Some(b) => b,
            None => return,
        };

        let mut envelopes = Vec::new();
        if let Some(consumer) = bridge.consumer.borrow().as_ref() {
            consumer.drain(|env| envelopes.push(env));
        }

        for env in envelopes {
            let cmd = env.command.clone();
            let response = self.dispatch_agent_command(cmd);
            env.reply(response);
        }
    }

    /// Compute `(should_advance, effective_dt)` honouring the agent
    /// pause / step machinery. The two callers (`update` real path and
    /// `end_agent_frame`-only path) share this so the semantics are
    /// in one place.
    fn compute_effective_dt(&self, delta_sec: f32) -> (bool, f32) {
        let Some(bridge) = self.agent.borrow().clone() else {
            return (true, delta_sec);
        };

        if !bridge.paused.get() {
            return (true, delta_sec);
        }

        let pending = bridge.requested_steps.get();
        if pending == 0 {
            return (false, 0.0);
        }

        bridge.requested_steps.set(pending - 1);
        (true, bridge.effective_step_dt())
    }

    /// Tail-end of every `update` tick: roll the synthetic input
    /// bridge, advance the frame counter, mirror fps/dt into the
    /// agent bridge so `/v1/state` reads the latest values without
    /// extra borrows.
    fn end_agent_frame(&self, dt: f32) {
        let Some(bridge) = self.agent.borrow().clone() else {
            return;
        };
        bridge.input_bridge.borrow().end_frame();
        bridge.frame.set(bridge.frame.get().saturating_add(1));
        bridge.dt_display.set(dt);
        // Mirror smoothed FPS using the same EMA the debug overlay
        // uses, so the two readouts stay coherent.
        let inst_fps = if dt > 1e-4 { 1.0 / dt } else { 0.0 };
        let prev = self.fps_smoothed.get();
        let fps = if prev <= 0.0 {
            inst_fps
        } else {
            prev * 0.9 + inst_fps * 0.1
        };
        self.fps_smoothed.set(fps);
        bridge.fps_display.set(fps);
    }

    /// Single switchboard for every [`AgentCommand`]. Runs on the game
    /// thread; safe to borrow `self.vm` / `self.agent`.
    fn dispatch_agent_command(&self, command: AgentCommand) -> AgentResponse {
        match command {
            AgentCommand::GetState => AgentResponse::State(self.build_state_snapshot()),
            AgentCommand::KeyInput(params) => self.handle_key_input(params),
            AgentCommand::AxisInput(params) => self.handle_axis_input(params),
            AgentCommand::TeleportPlayer(params) => self.handle_teleport(params),
            AgentCommand::AdvanceDialog => self.handle_advance_dialog(),
            AgentCommand::PauseTime => self.handle_pause(true),
            AgentCommand::ResumeTime => self.handle_pause(false),
            AgentCommand::StepTime(params) => self.handle_step(params),
            AgentCommand::FastForward(params) => self.handle_fast_forward(params),
            AgentCommand::SaveSlot(params) => {
                self.save_state(params.slot);
                AgentResponse::Ok
            }
            AgentCommand::LoadSlot(params) => {
                self.load_state(params.slot);
                AgentResponse::Ok
            }
            AgentCommand::LogTail(_) => {
                // Log tailing is served directly by the transport
                // layer from the global log sink; we should never see
                // one of these on the game thread.
                AgentResponse::err(AgentError::internal(
                    "log_tail must not be queued; served by transport",
                ))
            }
            AgentCommand::Screenshot => self.handle_screenshot(),
            AgentCommand::ScriptEval(params) => self.handle_script_eval(params),
            // `AgentCommand` is `#[non_exhaustive]` so future
            // additions don't break older sessions; until they're
            // wired here we fail closed with a clear error.
            _ => AgentResponse::err(AgentError::not_implemented(
                "command not implemented by the PAL4 session",
            )),
        }
    }

    fn handle_key_input(&self, params: KeyInputParams) -> AgentResponse {
        let Some(bridge) = self.agent.borrow().clone() else {
            return AgentResponse::err(AgentError::internal(
                "agent bridge unset while dispatching key input",
            ));
        };
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

    fn handle_axis_input(&self, params: AxisInputParams) -> AgentResponse {
        let Some(bridge) = self.agent.borrow().clone() else {
            return AgentResponse::err(AgentError::internal(
                "agent bridge unset while dispatching axis input",
            ));
        };
        let Some(axis) = Axis::from_name(&params.axis) else {
            return AgentResponse::err(AgentError::bad_request(format!(
                "unknown axis name: {}",
                params.axis
            )));
        };
        bridge.input_bridge.borrow().set_axis(axis, params.value);
        AgentResponse::Ok
    }

    fn handle_teleport(&self, params: TeleportParams) -> AgentResponse {
        let mut vm = self.vm.borrow_mut();
        vm.app_context_mut().set_player_pos(
            params.player,
            &Vec3::new(params.pos[0], params.pos[1], params.pos[2]),
        );
        AgentResponse::Ok
    }

    fn handle_advance_dialog(&self) -> AgentResponse {
        // Emulate the user pressing the dialog-advance key (Space) by
        // synthesizing a one-frame tap on the synthetic input bridge.
        // Falls through cleanly when no agent bridge is wired.
        if let Some(bridge) = self.agent.borrow().clone() {
            bridge.input_bridge.borrow().tap(Key::Space);
        }
        AgentResponse::Ok
    }

    fn handle_pause(&self, paused: bool) -> AgentResponse {
        let Some(bridge) = self.agent.borrow().clone() else {
            return AgentResponse::err(AgentError::internal("agent bridge unset"));
        };
        bridge.paused.set(paused);
        if !paused {
            // Resuming clears any leftover step budget so the game
            // doesn't double-tick.
            bridge.requested_steps.set(0);
        }
        AgentResponse::Ok
    }

    fn handle_step(&self, params: StepTimeParams) -> AgentResponse {
        let Some(bridge) = self.agent.borrow().clone() else {
            return AgentResponse::err(AgentError::internal("agent bridge unset"));
        };
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

    fn handle_fast_forward(&self, params: FastForwardParams) -> AgentResponse {
        self.vm
            .borrow_mut()
            .app_context_mut()
            .set_fast_forward(params.on);
        AgentResponse::Ok
    }

    fn handle_screenshot(&self) -> AgentResponse {
        let bridge = match self.agent.borrow().clone() {
            Some(b) => b,
            None => {
                return AgentResponse::err(AgentError::internal(
                    "agent bridge unset while dispatching screenshot",
                ))
            }
        };

        let engine_handle = bridge.rendering_engine.borrow().clone();
        let engine = match engine_handle {
            Some(e) => e,
            None => {
                return AgentResponse::err(AgentError::not_implemented(
                    "no rendering engine wired to the agent bridge (headless build)",
                ))
            }
        };

        let captured = engine.borrow_mut().capture_last_frame();
        match captured {
            Some(frame) => AgentResponse::Screenshot(ScreenshotResponse {
                width: frame.width,
                height: frame.height,
                encoded: true,
                rgba: frame.rgba,
            }),
            None => {
                // Empty payload — the transport's `respond_screenshot`
                // converts an empty ScreenshotResponse into a 501 with
                // a descriptive message, so we stay consistent with
                // every other "screenshot unavailable" path.
                AgentResponse::Screenshot(ScreenshotResponse::default())
            }
        }
    }

    fn handle_script_eval(&self, params: ScriptEvalParams) -> AgentResponse {
        // Script eval is a thin shim that returns a "not implemented"
        // until the per-function whitelist is wired in a follow-up:
        // implementing it requires marshalling JSON args back to the
        // AngelScript stack types, which is best done as a separate
        // patch with its own tests.
        AgentResponse::err(AgentError::not_implemented(format!(
            "script_eval for {} not yet implemented",
            params.function
        )))
        .also_record(&params)
    }

    fn build_state_snapshot(&self) -> StateSnapshot {
        let bridge = self.agent.borrow().clone();
        let vm = self.vm.borrow();
        let app = vm.app_context();
        let dialog = app.dialog_snapshot();

        let avatar_str = match dialog.avatar {
            DialogAvatarSide::Left => "left",
            DialogAvatarSide::Right => "right",
        };

        let leader_pos = app.leader_pos();
        let party = app
            .party_snapshot()
            .into_iter()
            .map(|p| PartyMember {
                slot: p.slot,
                level: p.level,
                hp: p.hp,
                max_hp: p.max_hp,
                mp: p.mp,
                max_mp: p.max_mp,
                in_team: p.in_team,
            })
            .collect();

        let paused = bridge.as_ref().map(|b| b.paused.get()).unwrap_or(false);
        let fps = bridge.as_ref().map(|b| b.fps_display.get()).unwrap_or(0.0);
        let dt = bridge.as_ref().map(|b| b.dt_display.get()).unwrap_or(0.0);
        let frame = bridge.as_ref().map(|b| b.frame.get()).unwrap_or(0);

        let current_script_fn = vm.current_function_name();

        StateSnapshot {
            frame,
            scene: app.scene_name().to_string(),
            block: app.block_name().to_string(),
            leader: app.leader() as i32,
            leader_pos: [leader_pos.x, leader_pos.y, leader_pos.z],
            party,
            money: app.persistent_state().money(),
            quest_percentage: app.persistent_state().quest_percentage(),
            dialog: DialogSnapshot {
                open: dialog.open,
                text: dialog.text,
                avatar: avatar_str.to_string(),
            },
            fast_forward: app.fast_forward(),
            paused,
            current_script_fn,
            fps,
            dt,
        }
    }
}

/// Tiny `AgentResponse` extension that lets `handle_script_eval` log
/// without disrupting its `match`-arm composition. Kept local to the
/// director so it doesn't leak as public API.
trait AgentResponseExt {
    fn also_record(self, params: &ScriptEvalParams) -> Self;
}

impl AgentResponseExt for AgentResponse {
    fn also_record(self, params: &ScriptEvalParams) -> Self {
        log::debug!("agent: script_eval unsupported for {}", params.function);
        self
    }
}
