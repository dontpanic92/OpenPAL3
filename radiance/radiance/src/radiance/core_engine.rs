use crosscom::ComRc;

use super::immediate_pump::ImmediateDirectorPump;
use super::ui_manager::UiManager;
use super::{DebugLayer, TaskManager};
use crate::asset::AssetManager;
use crate::comdef::{ISceneExt, ISceneManager};
use crate::rendering::{self, RenderingEngine};
use crate::{
    audio::AudioEngine,
    input::{InputEngine, InputEngineInternal},
};
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
};

pub struct CoreRadianceEngine {
    rendering_engine: Rc<RefCell<dyn RenderingEngine>>,
    audio_engine: Rc<dyn AudioEngine>,
    input_engine: Rc<RefCell<dyn InputEngineInternal>>,
    ui_manager: Rc<UiManager>,
    scene_manager: ComRc<ISceneManager>,
    /// Engine-owned asset / virtual-filesystem handle. Replaces the
    /// previous `Rc<RefCell<MiniFs>>` field that was dead code (no
    /// caller ever read it). App boot installs a fully-mounted
    /// `AssetManager` via [`CoreRadianceEngine::set_assets`] — until
    /// then this is an empty case-insensitive AssetManager so
    /// `assets()` always has something safe to return.
    assets: Rc<AssetManager>,
    task_manager: Rc<TaskManager>,
    services: RefCell<HashMap<TypeId, Rc<dyn Any>>>,
    debug_layer: Option<Box<dyn DebugLayer>>,
    /// Per-frame pump invoked inside the imgui scope when the
    /// active director needs immediate-mode rendering. See
    /// [`ImmediateDirectorPump`].
    immediate_pump: RefCell<Option<Rc<dyn ImmediateDirectorPump>>>,
}

impl CoreRadianceEngine {
    pub(crate) fn new(
        rendering_engine: Rc<RefCell<dyn RenderingEngine>>,
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngineInternal>>,
        ui_manager: Rc<UiManager>,
        scene_manager: ComRc<ISceneManager>,
    ) -> Self {
        Self {
            rendering_engine,
            audio_engine,
            input_engine,
            ui_manager,
            scene_manager,
            assets: AssetManager::new(),
            task_manager: Rc::new(TaskManager::new()),
            services: RefCell::new(HashMap::new()),
            debug_layer: None,
            immediate_pump: RefCell::new(None),
        }
    }

    pub fn rendering_component_factory(&self) -> Rc<dyn rendering::ComponentFactory> {
        self.rendering_engine.borrow().component_factory()
    }

    /// Shared handle to the rendering engine. Exposed so editor-style
    /// hosts can drive `render_scene_to_target` against offscreen
    /// `RenderTarget`s outside of the normal `update` flow.
    pub fn rendering_engine(&self) -> Rc<RefCell<dyn RenderingEngine>> {
        self.rendering_engine.clone()
    }

    pub fn audio_engine(&self) -> Rc<dyn AudioEngine> {
        self.audio_engine.clone()
    }

    pub fn input_engine(&self) -> Rc<RefCell<dyn InputEngine>> {
        self.input_engine.borrow().as_input_engine()
    }

    /// Engine-owned asset manager. Replaces the legacy `virtual_fs()`
    /// accessor (dead code, zero callers, removed). App boot can swap
    /// in a fully-mounted manager via [`Self::set_assets`].
    pub fn assets(&self) -> Rc<AssetManager> {
        self.assets.clone()
    }

    /// Install a fully-mounted `AssetManager` — call this from app
    /// boot after building the manager from
    /// `packfs::init_virtual_fs(...)` (wrap the resulting `MiniFs`
    /// with `AssetManager::from_mini_fs(...)`).
    pub fn set_assets(&mut self, assets: Rc<AssetManager>) {
        self.assets = assets;
    }

    pub fn set_debug_layer(&mut self, debug_layer: Box<dyn DebugLayer>) {
        self.debug_layer = Some(debug_layer);
    }

    pub fn scene_manager(&self) -> ComRc<ISceneManager> {
        self.scene_manager.clone()
    }

    pub fn ui_manager(&self) -> Rc<UiManager> {
        self.ui_manager.clone()
    }

    pub fn task_manager(&self) -> Rc<TaskManager> {
        self.task_manager.clone()
    }

    /// Install a per-frame immediate-mode director pump (see
    /// [`ImmediateDirectorPump`]). Replaces any previously-installed
    /// pump. The pump is invoked inside the imgui frame scope on
    /// each [`CoreRadianceEngine::update`] when both the pump slot
    /// and the scene manager's director are set.
    pub fn set_immediate_director_pump(&self, pump: Rc<dyn ImmediateDirectorPump>) {
        *self.immediate_pump.borrow_mut() = Some(pump);
    }

    /// Clear the previously-installed immediate-mode director pump.
    pub fn clear_immediate_director_pump(&self) {
        *self.immediate_pump.borrow_mut() = None;
    }

    pub fn set_service<T: Any>(&self, service: Rc<T>) -> Option<Rc<T>> {
        self.services
            .borrow_mut()
            .insert(TypeId::of::<T>(), service)
            .and_then(|service| service.downcast::<T>().ok())
    }

    pub fn service<T: Any>(&self) -> Option<Rc<T>> {
        self.services
            .borrow()
            .get(&TypeId::of::<T>())
            .cloned()
            .and_then(|service| service.downcast::<T>().ok())
    }

    pub fn get_or_insert_service<T: Any>(&self, create: impl FnOnce() -> T) -> Rc<T> {
        if let Some(service) = self.service::<T>() {
            return service;
        }

        let service = Rc::new(create());
        self.set_service(service.clone());
        service
    }

    pub fn update(&self, delta_sec: f32) {
        let frame_start = std::time::Instant::now();
        let phase_start = std::time::Instant::now();
        self.rendering_engine.borrow_mut().begin_frame();
        crate::perf::count(
            "engine.begin_frame_total_ns",
            phase_start.elapsed().as_nanos() as u64,
        );

        let phase_start = std::time::Instant::now();
        self.input_engine.borrow_mut().update(delta_sec);
        crate::perf::count(
            "engine.input_update_total_ns",
            phase_start.elapsed().as_nanos() as u64,
        );

        let scene_manager = self.scene_manager.clone();
        let task_manager = self.task_manager.clone();
        let debug_layer = self.debug_layer.as_ref();
        // Snapshot the pump out of the RefCell *before* entering
        // the ui_manager closure so a pump impl that re-enters
        // engine methods doesn't double-borrow `immediate_pump`.
        let pump = self.immediate_pump.borrow().clone();
        let phase_start = std::time::Instant::now();
        let ui_frame = self.ui_manager.update(delta_sec, |ui| {
            // Fire the immediate-mode director pump *before*
            // scene_manager.update so the active director's
            // `render_im` runs in render-then-update order
            // (matching the legacy `ScriptedImmediateDirector`
            // semantics). A transition produced by
            // `scene_manager.update`'s `director.update` call will
            // first appear on the next frame's `render_im`.
            if let Some(pump) = pump.as_ref() {
                if let Some(director) = scene_manager.director() {
                    pump.pump(director, delta_sec);
                }
            }
            let scene_update_start = std::time::Instant::now();
            scene_manager.update(delta_sec);
            crate::perf::count(
                "engine.scene_update_total_ns",
                scene_update_start.elapsed().as_nanos() as u64,
            );
            task_manager.update(delta_sec);

            if let Some(dl) = debug_layer {
                dl.update(delta_sec);
            }
            let _ = ui;
        });
        crate::perf::count(
            "engine.ui_manager_update_total_ns",
            phase_start.elapsed().as_nanos() as u64,
        );

        use crate::{math::Rect, scene::Viewport};
        let phase_start = std::time::Instant::now();
        let scene = self.scene_manager.scene();
        if let Some(s) = scene {
            let mut rendering_engine = self.rendering_engine.as_ref().borrow_mut();
            let viewport = {
                let mut camera = s.camera_mut();
                match camera.viewport() {
                    Viewport::FullExtent(_) => {
                        let extent = rendering_engine.view_extent();
                        camera.set_viewport(Viewport::FullExtent(Rect {
                            x: 0.,
                            y: 0.,
                            width: extent.0 as f32,
                            height: extent.1 as f32,
                        }));
                        camera.set_aspect(extent.0 as f32 / extent.1 as f32)
                    }
                    Viewport::CustomViewport(r) => {
                        camera.set_aspect(r.width as f32 / r.height as f32);
                    }
                }

                camera.viewport()
            };

            rendering_engine.render(s, viewport, ui_frame);
        }
        crate::perf::count(
            "engine.scene_render_total_ns",
            phase_start.elapsed().as_nanos() as u64,
        );

        let phase_start = std::time::Instant::now();
        self.rendering_engine.borrow_mut().end_frame();
        crate::perf::count(
            "engine.end_frame_total_ns",
            phase_start.elapsed().as_nanos() as u64,
        );

        crate::perf::count(
            "engine.update_total_ns",
            frame_start.elapsed().as_nanos() as u64,
        );
    }
}
