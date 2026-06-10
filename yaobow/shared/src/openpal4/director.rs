use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use agent_server::protocol::{
    AgentCommand, AgentError, AgentResponse, DialogSnapshot, FastForwardParams, FireTriggerParams,
    NameParams, NpcEntry, ObjectEntry, PartyMember, SceneObjectsResponse, SceneTriggersResponse,
    ScriptEvalParams, ScriptGlobalsParams, ScriptGlobalsResponse, StateSnapshot, TeleportParams,
    TriggerEntry,
};
use crosscom::ComRc;
use radiance::comdef::{IEntityExt, IImmediateDirectorImpl, IUiHost};
use radiance::math::Vec3;
use radiance::{
    audio::AudioEngine,
    comdef::{IDirectorImpl, ISceneManager},
    input::{InputEngine, Key},
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
    session::Pal4Session,
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

    /// In-flight `fire_trigger { wait_until_idle: true }` requests
    /// whose dispatcher took ownership of the [`AgentEnvelope`] and
    /// will reply once the VM settles (or the configured timeout
    /// elapses). Polled once per `update` after the VM has ticked.
    /// Always empty when no agent bridge is installed.
    pending_fires: RefCell<Vec<PendingFire>>,

    /// Save slot queued by the start-menu load screen
    /// (`Pal4Service::enter_load_game`). Applied via
    /// [`load_state`] on the first advancing `update`, then cleared,
    /// so the saved scene/leader/position and story-plot globals are
    /// restored before play begins. `None` for a fresh New Game.
    pending_load_slot: Cell<Option<i32>>,
}

/// How often the debug overlay republishes the FPS / frame-time
/// readouts. Tuned so the numbers are readable rather than strobing.
const DEBUG_METRIC_INTERVAL_SEC: f32 = 0.5;

/// Number of consecutive idle frames a deferred fire must observe
/// before the dispatcher declares it "settled". Rides out the
/// momentary blip between two scripted continuations (e.g. talk →
/// camera pan → NPC walk) so the planner sees a truly quiescent VM.
const IDLE_SETTLE_FRAMES: u32 = 2;
/// Default timeout (ms) for a `fire_trigger { wait_until_idle }` call.
const DEFAULT_FIRE_TIMEOUT_MS: u64 = 5_000;
/// Hard cap (ms) on the same — prevents a buggy planner from pinning
/// game-thread memory for the full agent reply window. PAL4 cutscenes
/// in M01/Q01 routinely chain 30+ `giPlayerDoAction` /
/// `giPlayerEndAction` pairs; with multiple actors each running 4-8s
/// animations the total cutscene length can exceed 2 minutes even
/// with fast-forward (`giWait` is the only sysfn fast-forward affects).
const MAX_FIRE_TIMEOUT_MS: u64 = 180_000;

/// One in-flight `fire_trigger { wait_until_idle: true }` request.
/// Held in [`OpenPAL4Director::pending_fires`] until the VM settles
/// or the deadline expires.
struct PendingFire {
    env: agent_server::AgentEnvelope,
    name: String,
    collect_trace: bool,
    start_seq: u64,
    waited_frames: u32,
    idle_streak: u32,
    max_frames: u32,
}

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
        session: Rc<RefCell<Pal4Session>>,
    ) -> Self {
        // The session is the app-lifetime, Rc-shared playthrough state
        // owned by `Pal4Service`; this director just holds a clone of
        // the handle in its context, so the same session is observed by
        // every mode and by app-lifetime code (the agent dispatcher).
        let app_context = Pal4AppContext::new(
            component_factory,
            loader,
            scene_manager,
            ui,
            input.clone(),
            audio,
            task_manager,
            session,
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
            pending_fires: RefCell::new(Vec::new()),
            pending_load_slot: Cell::new(None),
        }
    }

    /// Install the agent-server bridge. Idempotent — passing a new
    /// bridge replaces the previous one. Must be called *before* the
    /// engine starts pumping `update` for the per-frame drain to
    /// pick up any commands the HTTP listener has queued.
    pub fn set_agent_bridge(&self, bridge: Rc<Pal4AgentBridge>) {
        *self.agent.borrow_mut() = Some(bridge);
    }

    /// Queue a save-slot load to be applied on the first advancing
    /// `update`. Used by `Pal4Service::enter_load_game`
    /// so the start-menu load screen can boot directly into a save.
    pub fn set_pending_load_slot(&self, slot: i32) {
        self.pending_load_slot.set(Some(slot));
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
    /// shared angelscript globals (story-plot flags) plus the leader's
    /// live position / facing and camera so a later load resumes at the
    /// same point. Scene / block / leader / player-locked already live
    /// in the session (its single source of truth — see
    /// `Pal4AppContext`), so they are persisted automatically. Negative
    /// slots are ignored by `Pal4PersistentState::save`.
    pub fn save_state(&self, slot: i32) {
        let mut vm = self.vm.borrow_mut();
        let globals = vm.g.borrow().globals_snapshot();

        let app = vm.app_context_mut();
        let position = Some(app.leader_pos());
        let direction = Some(app.leader_direction());
        let camera = app.camera_transform();

        app.session_mut()
            .save_runtime(slot, position, direction, camera, globals);
    }

    /// Restore game state from `slot`, reloading the saved scene and
    /// repositioning the leader. No-ops (with a log) when the slot file
    /// is missing or malformed.
    pub fn load_state(&self, slot: i32) {
        let mut vm = self.vm.borrow_mut();

        let loaded = vm.app_context_mut().session_mut().load_slot(slot);
        let snapshot = match loaded {
            Ok(snapshot) => snapshot,
            Err(e) => {
                log::error!("Cannot load save slot {}: {}", slot, e);
                return;
            }
        };

        vm.g.borrow_mut().restore_globals(&snapshot.script_globals);

        let app = vm.app_context_mut();

        if !snapshot.scene_name.is_empty() {
            if let Err(err) = app.load_scene(&snapshot.scene_name, &snapshot.block_name) {
                log::error!(
                    "OpenPAL4Director::load_state: failed to load scene='{}' block='{}': {:?}; \
                     save-load partially restored (globals + leader applied, scene not changed)",
                    snapshot.scene_name,
                    snapshot.block_name,
                    err
                );
                return;
            }
            app.set_leader(snapshot.leader as i32);
            if let Some(pos) = snapshot.position {
                app.set_player_pos(snapshot.leader as i32, &pos);
            }
            if let Some(dir) = snapshot.direction {
                app.player_set_direction(snapshot.leader as i32, dir);
            }
            // Restore the exact camera view last, after the scene (and
            // its fresh default camera) is in place. Older saves with
            // no camera snapshot keep the scene default.
            if let Some(camera) = snapshot.camera {
                app.set_camera_transform(&camera);
            }
            // Re-apply the saved control-lock state to the new scene's
            // actor controller. Without this the player stays locked at
            // the app-context default (locked), since we cancelled the
            // new-game opening script that would normally unlock it.
            app.lock_player(snapshot.player_locked);
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
        let _timer = radiance::perf::timer("pal4.director.update_total_ns");
        // Agent commands were already drained this frame by the
        // app-lifetime `Pal4AgentDispatcher` in `on_updating` (which
        // runs before this `update`), so `pause` / `step` / `input` /
        // `fire_trigger` requests are already applied here. This
        // director only owns the post-VM settle (`poll_pending_fires`)
        // and the per-frame agent bookkeeping (`end_agent_frame`).

        // Decide whether this real frame actually advances the
        // simulation. Pause + zero pending steps = full freeze
        // (input bridge still ticks so a held key set just before
        // pause stays sensible; that's handled below).
        let (advance, effective_dt) = self.compute_effective_dt(delta_sec);

        if !advance {
            self.end_agent_frame();
            return None;
        }

        self.vm.borrow_mut().app_context_mut().update(effective_dt);

        // Apply a pending start-menu "Load Game" selection on the
        // first advancing frame, before any new-game script triggers.
        // Done here (not in `activate`) so the VM + scene stack are
        // ready, matching the in-game save/load hotkey path below.
        //
        // The story VM boots with the new-game opening script
        // (function index 0) already queued, so after restoring the
        // save we must cancel it — otherwise it executes below and
        // overrides the loaded scene, dropping the player into a fresh
        // new game. Resetting the VM to idle lets `event_triggered`
        // drive the restored scene's triggers, exactly like an
        // in-game (idle) save load.
        if let Some(slot) = self.pending_load_slot.take() {
            self.load_state(slot);
            self.vm.borrow_mut().reset_to_idle();
        }

        self.poll_save_load_hotkeys();

        if self.vm.borrow().context.is_none() {
            let function = radiance::perf::time("pal4.director.event_triggered_total_ns", || {
                self.vm
                    .borrow_mut()
                    .app_context_mut()
                    .event_triggered(effective_dt)
            });
            if let Some(function) = function {
                let module = self.vm.borrow().app_context.scene.module.clone().unwrap();
                self.vm
                    .borrow_mut()
                    .set_function_by_name2(module, &function);
            }
        }

        radiance::perf::time("pal4.vm.execute_total_ns", || {
            self.vm.borrow_mut().execute(effective_dt);
        });

        // Reply to any deferred `fire_trigger { wait_until_idle }`
        // calls now that the VM has had its tick this frame. Done
        // after `execute` so the settle observation sees the latest
        // VM state.
        self.poll_pending_fires();

        /*if !self.vm.borrow().app_context().player_locked {
            self.control
                .update(scene_manager.scene().unwrap(), delta_sec)
        }*/

        self.end_agent_frame();

        None
    }

    fn deactivate(&self) {
        // Fail any in-flight deferred `fire_trigger { wait_until_idle }`
        // replies: this director is being torn down (e.g. an agent
        // `new_game` / `load`), so their VM will never settle. Reply each
        // with a clear error instead of letting the reply channels drop
        // to a generic "channel disconnected" 500.
        let pending = std::mem::take(&mut *self.pending_fires.borrow_mut());
        for pf in pending {
            pf.env.reply(AgentResponse::err(AgentError::conflict(
                "mode changed before trigger settled",
            )));
        }

        // Nothing else to do for the agent surface: the app-lifetime
        // `Pal4AgentDispatcher` looks up the active director from the
        // `SceneManager` each frame, so once this director is no longer
        // installed the dispatcher automatically falls back to the
        // ambient (menu) command set — no flag to clear.
        //
        // Nothing to do for the session either: it is owned at app
        // lifetime by `Pal4Service` via a shared `Rc` handle. Dropping
        // this director just drops its handle clone; the session lives
        // on, so a future mode resumes the same state.
    }
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
    /// Handle one agent command envelope with full story-mode
    /// capabilities (VM, scene, app_context). The app-lifetime
    /// [`Pal4AgentDispatcher`](crate::openpal4::service::Pal4Service::pump_agent)
    /// drains the queue once per frame (in `on_updating`, before this
    /// director's `update`) and calls this for each envelope when the
    /// active director is a story director. `FireSceneTrigger` with
    /// `wait_until_idle` is special-cased into the cross-frame settle
    /// queue (replied by [`Self::poll_pending_fires`] after the VM
    /// ticks); every other command replies synchronously here.
    pub fn handle_agent_envelope(&self, env: agent_server::AgentEnvelope) {
        if let agent_server::protocol::AgentCommand::FireSceneTrigger(params) = &env.command {
            if params.wait_until_idle {
                self.begin_pending_fire(params.clone(), env);
                return;
            }
        }
        let cmd = env.command.clone();
        let response = self.dispatch_agent_command(cmd);
        env.reply(response);
    }

    /// Poll the deferred-reply queue and ship responses for any
    /// fires that have settled or timed out. Called once per
    /// `update` *after* the VM tick so the latest VM state is
    /// observable here.
    fn poll_pending_fires(&self) {
        let bridge = match self.agent.borrow().clone() {
            Some(b) => b,
            None => return,
        };
        let mut keep: Vec<PendingFire> = Vec::new();
        let drained = std::mem::take(&mut *self.pending_fires.borrow_mut());
        let vm = self.vm.borrow();
        let now_seq = bridge.trace_sink.drain(u64::MAX, 0).next_seq;
        let script_idle = vm.context.is_none();
        drop(vm);
        for mut pf in drained {
            pf.waited_frames = pf.waited_frames.saturating_add(1);
            // Require `idle_settle_frames` consecutive idle frames
            // before declaring "settled". This rides out the racy
            // window between two scripted continuations (e.g. talk
            // -> camera pan -> NPC walk) so the planner observes a
            // truly quiescent state, not a momentary blip.
            if script_idle {
                pf.idle_streak = pf.idle_streak.saturating_add(1);
            } else {
                pf.idle_streak = 0;
            }
            let timed_out = pf.waited_frames >= pf.max_frames;
            let settled = pf.idle_streak >= IDLE_SETTLE_FRAMES;
            if settled || timed_out {
                let trace_seq_start = if pf.collect_trace {
                    Some(pf.start_seq)
                } else {
                    None
                };
                let trace_seq_end = if pf.collect_trace {
                    Some(now_seq)
                } else {
                    None
                };
                let current_fn = self.vm.borrow().current_function_name();
                let response =
                    AgentResponse::FireTrigger(agent_server::protocol::FireTriggerResponse {
                        name: pf.name,
                        settled,
                        trace_seq_start,
                        trace_seq_end,
                        waited_frames: pf.waited_frames,
                        current_script_fn: current_fn,
                    });
                pf.env.reply(response);
            } else {
                keep.push(pf);
            }
        }
        *self.pending_fires.borrow_mut() = keep;
    }

    /// Kick off the script and stash the envelope for the
    /// deferred-reply poll. Failure modes (no scene, unknown
    /// trigger, currently-running script) short-circuit with an
    /// immediate reply.
    fn begin_pending_fire(
        &self,
        params: agent_server::protocol::FireTriggerParams,
        env: agent_server::AgentEnvelope,
    ) {
        let bridge = match self.agent.borrow().clone() {
            Some(b) => b,
            None => {
                env.reply(AgentResponse::err(AgentError::internal(
                    "agent bridge unset while firing trigger",
                )));
                return;
            }
        };
        // Snapshot the trace cursor *before* dispatch so the planner
        // can drain just this fire's events.
        let start_seq = bridge.trace_sink.drain(u64::MAX, 0).next_seq;
        let immediate = self.handle_fire_scene_trigger(params.clone());
        // If the fire failed synchronously (unknown trigger, etc.)
        // bail with that error rather than queueing a settle.
        match &immediate {
            AgentResponse::Ok => {}
            _ => {
                env.reply(immediate);
                return;
            }
        }
        let timeout = params
            .timeout_ms
            .map(|ms| ms.min(MAX_FIRE_TIMEOUT_MS))
            .unwrap_or(DEFAULT_FIRE_TIMEOUT_MS);
        // Convert ms to frames at the engine's nominal tick rate so
        // the deadline travels in lockstep with `update`. Round up.
        let max_frames = (timeout.saturating_mul(60) + 999) / 1000;
        self.pending_fires.borrow_mut().push(PendingFire {
            env,
            name: params.name,
            collect_trace: params.collect_trace,
            start_seq,
            waited_frames: 0,
            idle_streak: 0,
            max_frames: max_frames as u32,
        });
    }

    /// Compute `(should_advance, effective_dt)` honouring the agent
    /// pause / step machinery. The policy itself lives on
    /// [`Pal4AgentBridge::effective_dt`] (generic, mode-agnostic); this
    /// just applies it, or runs at the real rate when no agent bridge
    /// is installed.
    fn compute_effective_dt(&self, delta_sec: f32) -> (bool, f32) {
        self.agent
            .borrow()
            .as_ref()
            .map_or((true, delta_sec), |bridge| bridge.effective_dt(delta_sec))
    }

    /// Tail-end of every story `update` tick: clear the synthetic-input
    /// edges now that the VM / scripts / actor controllers have polled
    /// input for the frame. The frame-counter / fps / dt **telemetry**
    /// is published separately (once per frame, for every mode) by the
    /// app-lifetime dispatcher via
    /// [`Pal4AgentBridge::publish_frame_telemetry`] — only the
    /// input-edge clear has to stay here, because it must run *after*
    /// this director ticks its VM.
    fn end_agent_frame(&self) {
        if let Some(bridge) = self.agent.borrow().as_ref() {
            bridge.input_bridge.borrow().end_frame();
        }
    }

    /// Switchboard for the **VM / scene** agent commands — the ones that
    /// need the running script VM, the live scene, or `Pal4AppContext`.
    /// Bridge-only commands (input, time control, screenshot, perf, log)
    /// are handled upstream by the app-lifetime `Pal4AgentDispatcher`
    /// (`Pal4Service::dispatch_bridge_command`) and never reach here.
    /// Runs on the game thread; safe to borrow `self.vm` / `self.agent`.
    fn dispatch_agent_command(&self, command: AgentCommand) -> AgentResponse {
        match command {
            AgentCommand::GetState => AgentResponse::State(self.build_state_snapshot()),
            AgentCommand::TeleportPlayer(params) => self.handle_teleport(params),
            AgentCommand::AdvanceDialog => self.handle_advance_dialog(),
            AgentCommand::FastForward(params) => self.handle_fast_forward(params),
            AgentCommand::SaveSlot(params) => {
                self.save_state(params.slot);
                AgentResponse::Ok
            }
            AgentCommand::LoadSlot(params) => {
                self.load_state(params.slot);
                AgentResponse::Ok
            }
            AgentCommand::ScriptEval(params) => self.handle_script_eval(params),
            AgentCommand::GetSceneTriggers => self.handle_get_scene_triggers(),
            AgentCommand::GetSceneObjects => self.handle_get_scene_objects(),
            AgentCommand::GetScriptGlobals(params) => self.handle_get_script_globals(params),
            AgentCommand::FireSceneTrigger(params) => self.handle_fire_scene_trigger(params),
            AgentCommand::InteractObject(params) => self.handle_interact_object(params),
            AgentCommand::TraceStart(params) => self.handle_trace_start(params),
            AgentCommand::TraceStop => self.handle_trace_stop(),
            AgentCommand::TraceDrain(params) => self.handle_trace_drain(params),
            AgentCommand::ChooseDialog(params) => {
                self.vm
                    .borrow_mut()
                    .app_context_mut()
                    .buffer_dialog_choice(params.index);
                AgentResponse::Ok
            }
            AgentCommand::ChooseWorldMap(params) => {
                self.vm
                    .borrow()
                    .app_context()
                    .buffer_world_map_choice(params.scene, params.block);
                AgentResponse::Ok
            }
            // Bridge-only commands (input / time / screenshot / perf /
            // log) are handled by the dispatcher and never routed here;
            // `AgentCommand` is also `#[non_exhaustive]`. Either way we
            // fail closed with a clear error.
            _ => AgentResponse::err(AgentError::not_implemented(
                "command not implemented by the PAL4 session",
            )),
        }
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

    fn handle_fast_forward(&self, params: FastForwardParams) -> AgentResponse {
        self.vm
            .borrow_mut()
            .app_context_mut()
            .set_fast_forward(params.on);
        AgentResponse::Ok
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

    /// Snapshot the EVF event triggers for the currently loaded
    /// block. Returns an empty list with `scene`/`block` = `""` while
    /// the placeholder scene is active.
    fn handle_get_scene_triggers(&self) -> AgentResponse {
        let vm = self.vm.borrow();
        let app = vm.app_context();

        let triggers = app
            .scene
            .events
            .iter()
            .map(|event| {
                let (center, half_size) = trigger_bounds(event);
                let shape = match event.vertex_count {
                    4 => "plane",
                    8 => "box",
                    _ => "other",
                };
                TriggerEntry {
                    name: event.name.to_string().unwrap_or_default(),
                    function: event.function.function.to_string().unwrap_or_default(),
                    center,
                    half_size,
                    shape: shape.to_string(),
                }
            })
            .collect();

        AgentResponse::SceneTriggers(SceneTriggersResponse {
            scene: app.scene_name().to_string(),
            block: app.block_name().to_string(),
            triggers,
        })
    }

    /// Snapshot the GOB objects + NPCs for the currently loaded
    /// block, with each entry's live world-space position.
    fn handle_get_scene_objects(&self) -> AgentResponse {
        let vm = self.vm.borrow();
        let app = vm.app_context();

        let npcs = app
            .scene
            .npcs
            .iter()
            .map(|entity| {
                let pos = entity.world_transform().position();
                NpcEntry {
                    name: entity.name(),
                    position: [pos.x, pos.y, pos.z],
                    visible: entity.visible(),
                }
            })
            .collect::<Vec<_>>();

        let objects = match app.scene.objects_gob.as_ref() {
            Some(gob) => gob
                .entries
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let name = entry.name.to_string().unwrap_or_default();
                    let kind = gob
                        .header
                        .object_types
                        .get(i)
                        .and_then(|t| fileformats::pal4::gob::GobObjectType::name(*t))
                        .unwrap_or("unknown")
                        .to_string();
                    let research_function = entry.research_function.to_string().unwrap_or_default();
                    // Prefer the live entity position (scripts may have
                    // moved or hidden the object), fall back to the
                    // GOB load-time position for entries we never
                    // instantiated (EFFECT entries, failed loads).
                    let (position, visible) = match app.scene.get_object(&name) {
                        Some(entity) => {
                            let p = entity.world_transform().position();
                            ([p.x, p.y, p.z], entity.visible())
                        }
                        None => (entry.position, false),
                    };
                    ObjectEntry {
                        name,
                        kind,
                        position,
                        visible,
                        research_function,
                    }
                })
                .collect(),
            None => Vec::new(),
        };

        AgentResponse::SceneObjects(SceneObjectsResponse {
            scene: app.scene_name().to_string(),
            block: app.block_name().to_string(),
            npcs,
            objects,
        })
    }

    /// Return a window over the shared AngelScript globals. The
    /// underlying array doesn't change size at runtime, so `len` is
    /// stable per module load; clients diff `globals[]` to detect
    /// plot progression.
    fn handle_get_script_globals(&self, params: ScriptGlobalsParams) -> AgentResponse {
        let vm = self.vm.borrow();
        let snap = vm.g.borrow().globals_snapshot();
        let len = snap.len();
        let start = params.start.min(len);
        let end = match params.limit {
            Some(limit) => start.saturating_add(limit).min(len),
            None => len,
        };
        AgentResponse::ScriptGlobals(ScriptGlobalsResponse {
            len,
            start,
            globals: snap[start..end].to_vec(),
        })
    }

    /// Fire an EVF trigger by name. Reuses the engine's own
    /// `set_function_by_name2` path so the script side is identical
    /// to a real collision.
    fn handle_fire_scene_trigger(&self, params: FireTriggerParams) -> AgentResponse {
        if self.vm.borrow().current_function_name().is_some() {
            return AgentResponse::err(AgentError::conflict(
                "a script is currently running; wait for it to finish before firing a trigger",
            ));
        }
        let (module, fn_name) = {
            let vm = self.vm.borrow();
            let app = vm.app_context();
            let Some(module) = app.scene.module.clone() else {
                return AgentResponse::err(AgentError::conflict(
                    "no scene is loaded; load a block before firing triggers",
                ));
            };
            let Some(event) = app
                .scene
                .events
                .iter()
                .find(|e| e.name.to_string().ok().as_deref() == Some(params.name.as_str()))
            else {
                return AgentResponse::err(AgentError::bad_request(format!(
                    "unknown trigger name: {}",
                    params.name
                )));
            };
            let fn_name = event.function.function.to_string().unwrap_or_default();
            if fn_name.is_empty() {
                return AgentResponse::err(AgentError::bad_request(format!(
                    "trigger {} has no script function bound",
                    params.name
                )));
            }
            (module, fn_name)
        };
        self.vm.borrow_mut().set_function_by_name2(module, &fn_name);
        AgentResponse::Ok
    }

    /// Fire a GOB entry's `research_function` as if the player had
    /// pressed "Examine" on it.
    fn handle_interact_object(&self, params: NameParams) -> AgentResponse {
        if self.vm.borrow().current_function_name().is_some() {
            return AgentResponse::err(AgentError::conflict(
                "a script is currently running; wait for it to finish before interacting",
            ));
        }
        let (module, fn_name) = {
            let vm = self.vm.borrow();
            let app = vm.app_context();
            let Some(module) = app.scene.module.clone() else {
                return AgentResponse::err(AgentError::conflict(
                    "no scene is loaded; load a block before interacting with objects",
                ));
            };
            let Some(gob) = app.scene.objects_gob.as_ref() else {
                return AgentResponse::err(AgentError::conflict(
                    "no GOB loaded for the current block",
                ));
            };
            let Some(entry) = gob
                .entries
                .iter()
                .find(|e| e.name.to_string().ok().as_deref() == Some(params.name.as_str()))
            else {
                return AgentResponse::err(AgentError::bad_request(format!(
                    "unknown object name: {}",
                    params.name
                )));
            };
            let fn_name = entry.research_function.to_string().unwrap_or_default();
            if fn_name.is_empty() {
                return AgentResponse::err(AgentError::bad_request(format!(
                    "object {} has no examine handler (research_function is empty)",
                    params.name
                )));
            }
            (module, fn_name)
        };
        self.vm.borrow_mut().set_function_by_name2(module, &fn_name);
        AgentResponse::Ok
    }

    /// Begin capturing the VM execution trace. Installs the agent
    /// bridge's [`AgentTraceAdapter`] on the script VM (idempotent —
    /// re-installation replaces the previous sink with the same one)
    /// and arms the underlying ring buffer.
    fn handle_trace_start(
        &self,
        params: agent_server::protocol::TraceStartParams,
    ) -> AgentResponse {
        let Some(bridge) = self.agent.borrow().clone() else {
            return AgentResponse::err(AgentError::internal(
                "agent bridge unset while starting trace",
            ));
        };
        bridge.trace_sink.start(params.reset, params.capacity);
        // Install the adapter as the VM's trace sink. Cheap, but only
        // needed once per session — subsequent starts only need to
        // re-arm the ring buffer.
        let adapter: std::rc::Rc<dyn crate::scripting::angelscript::TraceSink> =
            bridge.trace_adapter.clone();
        self.vm.borrow_mut().set_trace_sink(Some(adapter));
        AgentResponse::Ok
    }

    /// Stop capturing. The buffered events remain drainable until
    /// the ring evicts them or a subsequent [`Self::handle_trace_start`]
    /// resets the buffer.
    fn handle_trace_stop(&self) -> AgentResponse {
        let Some(bridge) = self.agent.borrow().clone() else {
            return AgentResponse::err(AgentError::internal(
                "agent bridge unset while stopping trace",
            ));
        };
        bridge.trace_sink.stop();
        AgentResponse::Ok
    }

    /// Drain buffered trace events with `seq > after_seq`.
    fn handle_trace_drain(
        &self,
        params: agent_server::protocol::TraceDrainParams,
    ) -> AgentResponse {
        let Some(bridge) = self.agent.borrow().clone() else {
            return AgentResponse::err(AgentError::internal(
                "agent bridge unset while draining trace",
            ));
        };
        let n = params.n.unwrap_or(1024);
        let result = bridge.trace_sink.drain(params.after_seq, n);
        AgentResponse::TraceDrain(agent_server::protocol::TraceDrainResponse {
            next_seq: result.next_seq,
            dropped: result.dropped,
            capturing: bridge.trace_sink.is_capturing(),
            events: result.events,
        })
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
        let script_running = current_script_fn.is_some();
        let movie_playing = app.movie_playing();
        let inventory = app
            .inventory_snapshot()
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .map(|(id, count)| agent_server::protocol::InventoryEntry { id, count })
            .collect();

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
                choices: app.dialog_choices().to_vec(),
            },
            fast_forward: app.fast_forward(),
            paused,
            current_script_fn,
            script_running,
            movie_playing,
            fps,
            dt,
            inventory,
            world_map_open: app.world_map_open(),
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

/// Compute a `(center, half_size)` bounding pair for an EVF event's
/// trigger volume by AABB-ing the vertex centers. Mirrors the engine's
/// own "skip if not 4 or 8 vertices" rule for ray-caster construction
/// (the bounds are still returned even for `other` shapes so an agent
/// can see the raw data — it just won't be collidable).
fn trigger_bounds(event: &fileformats::pal4::evf::EvfEvent) -> ([f32; 3], [f32; 3]) {
    if event.vertices.is_empty() {
        return ([0.0; 3], [0.0; 3]);
    }
    let first = &event.vertices[0];
    let mut min = [
        first.center.x as f32,
        first.center.y as f32,
        first.center.z as f32,
    ];
    let mut max = min;
    for v in event.vertices.iter().skip(1) {
        let p = [v.center.x as f32, v.center.y as f32, v.center.z as f32];
        for i in 0..3 {
            if p[i] < min[i] {
                min[i] = p[i];
            }
            if p[i] > max[i] {
                max[i] = p[i];
            }
        }
    }
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let half_size = [
        (max[0] - min[0]) * 0.5,
        (max[1] - min[1]) * 0.5,
        (max[2] - min[2]) * 0.5,
    ];
    (center, half_size)
}
