use crosscom::ComRc;
use mini_fs::MiniFs;

use super::ui_manager::UiManager;
use super::{DebugLayer, TaskManager};
use crate::comdef::ISceneManager;
use crate::rendering::{self, RenderingEngine};
use crate::{
    audio::AudioEngine,
    input::{InputEngine, InputEngineInternal},
};
use std::{cell::RefCell, rc::Rc};

pub struct CoreRadianceEngine {
    rendering_engine: Rc<RefCell<dyn RenderingEngine>>,
    audio_engine: Rc<dyn AudioEngine>,
    input_engine: Rc<RefCell<dyn InputEngineInternal>>,
    ui_manager: Rc<UiManager>,
    scene_manager: ComRc<ISceneManager>,
    virtual_fs: Rc<RefCell<MiniFs>>,
    task_manager: Rc<TaskManager>,
    debug_layer: Option<Box<dyn DebugLayer>>,
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
            virtual_fs: Rc::new(RefCell::new(MiniFs::new(false))),
            task_manager: Rc::new(TaskManager::new()),
            debug_layer: None,
        }
    }

    pub fn rendering_component_factory(&self) -> Rc<dyn rendering::ComponentFactory> {
        self.rendering_engine.borrow().component_factory()
    }

    pub fn audio_engine(&self) -> Rc<dyn AudioEngine> {
        self.audio_engine.clone()
    }

    pub fn input_engine(&self) -> Rc<RefCell<dyn InputEngine>> {
        self.input_engine.borrow().as_input_engine()
    }

    pub fn virtual_fs(&self) -> Rc<RefCell<MiniFs>> {
        self.virtual_fs.clone()
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

    pub fn update(&self, delta_sec: f32) {
        self.rendering_engine.borrow_mut().begin_frame();

        self.input_engine.borrow_mut().update(delta_sec);
        let scene_manager = self.scene_manager.clone();
        let task_manager = self.task_manager.clone();
        let debug_layer = self.debug_layer.as_ref();
        let ui_frame = self.ui_manager.update(delta_sec, |ui| {
            scene_manager.update(ui, delta_sec);
            task_manager.update(delta_sec);

            if let Some(dl) = debug_layer {
                dl.update(scene_manager.clone(), ui, delta_sec);
            }
        });

        use crate::{math::Rect, scene::Viewport};
        let scene = self.scene_manager.scene();
        if let Some(s) = scene {
            let mut rendering_engine = self.rendering_engine.as_ref().borrow_mut();
            let viewport = {
                let c = s.camera();
                let mut camera = c.borrow_mut();
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

        self.rendering_engine.borrow_mut().end_frame();
    }
}
