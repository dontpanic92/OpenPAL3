use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crosscom::ComRc;
use radiance::{
    audio::AudioEngine,
    comdef::{IDirectorImpl, ISceneManager},
    input::{InputEngine, Key},
    radiance::{TaskManager, UiManager},
    rendering::ComponentFactory,
    scene::CoreScene,
    utils::free_view::FreeViewController,
};
use radiance_scripting::comdef::immediate_director::{IImmediateDirectorImpl, IUiHost};

use crate::scripting::angelscript::ScriptVm;

use super::{
    app_context::Pal4AppContext,
    asset_loader::AssetLoader,
    comdef::{
        pal4_debug::{IPal4DebugContext, IPal4DebugOverlay},
        IOpenPAL4DirectorImpl,
    },
    pal4_debug::Pal4DebugContextInner,
    scripting::create_script_vm,
};

/// Bundle of script-side handles the director uses to drive the
/// protosept debug overlay each frame. Set by the application loader
/// after [`super::debug::create_context`] + the bootstrap script have
/// run; the director only reads from this bundle inside `render_im`.
pub struct Pal4DebugBundle {
    pub overlay: ComRc<IPal4DebugOverlay>,
    pub overlay_ctx: ComRc<IPal4DebugContext>,
    pub overlay_ctx_inner: Rc<Pal4DebugContextInner>,
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
        }
    }

    /// Install the protosept-authored debug overlay. Idempotent —
    /// passing a new bundle replaces the previous one. Safe to call
    /// after construction but before the engine starts pumping
    /// `render_im`.
    pub fn set_debug_bundle(&self, bundle: Pal4DebugBundle) {
        *self.debug.borrow_mut() = Some(bundle);
    }

    fn refresh_debug_snapshot(&self, delta_sec: f32) {
        let bundle_ref = self.debug.borrow();
        let Some(bundle) = bundle_ref.as_ref() else { return };

        // Exponential moving average so the FPS readout doesn't strobe
        // on jittery frames. alpha tuned to roughly half a second.
        let inst_fps = if delta_sec > 1e-4 { 1.0 / delta_sec } else { 0.0 };
        let prev = self.fps_smoothed.get();
        let fps = if prev <= 0.0 {
            inst_fps
        } else {
            prev * 0.9 + inst_fps * 0.1
        };
        self.fps_smoothed.set(fps);

        // Fan the script-side overlay toggles out to the live scene
        // before we snapshot, so the next overlay frame sees the
        // result of any button press from the previous frame. Done
        // per-frame so scene reloads pick up the current state with
        // no extra wiring.
        let bsp_visible = bundle.overlay_ctx_inner.bsp_visible();
        let nav_mesh_visible = bundle.overlay_ctx_inner.nav_mesh_visible();

        let mut vm = self.vm.borrow_mut();
        let app = vm.app_context_mut();
        app.set_bsp_visible(bsp_visible);
        app.set_nav_mesh_visible(nav_mesh_visible);

        let pos = app.leader_pos();
        bundle.overlay_ctx_inner.set_snapshot(super::pal4_debug::Pal4DebugSnapshot {
            scene_name: app.scene_name().to_string(),
            block_name: app.block_name().to_string(),
            leader_index: app.leader() as i32,
            leader_pos: [pos.x, pos.y, pos.z],
            delta_time: delta_sec,
            fps,
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
}

impl IOpenPAL4DirectorImpl for OpenPAL4Director {
    fn get(&self) -> &'static crate::openpal4::director::OpenPAL4Director {
        unsafe { &*(self as *const _) }
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
            let Some(bundle) = bundle_ref.as_ref() else { return };
            (bundle.overlay.clone(), bundle.overlay_ctx.clone())
        };
        overlay.render(ui, dt, ctx);
    }
}
