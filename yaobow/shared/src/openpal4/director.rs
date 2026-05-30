use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crosscom::ComRc;
use radiance::comdef::{IImmediateDirectorImpl, IUiHost};
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
    app_context::Pal4AppContext,
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
        }
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
            self.debug_metric_accum.set(accum - DEBUG_METRIC_INTERVAL_SEC);
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
        self.vm.borrow_mut().app_context_mut().update(delta_sec);

        self.poll_save_load_hotkeys();

        if self.vm.borrow().context.is_none() {
            let function = self
                .vm
                .borrow_mut()
                .app_context_mut()
                .event_triggered(delta_sec);
            if let Some(function) = function {
                let module = self.vm.borrow().app_context.scene.module.clone().unwrap();
                self.vm
                    .borrow_mut()
                    .set_function_by_name2(module, &function);
            }
        }

        self.vm.borrow_mut().execute(delta_sec);

        /*if !self.vm.borrow().app_context().player_locked {
            self.control
                .update(scene_manager.scene().unwrap(), delta_sec)
        }*/

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
