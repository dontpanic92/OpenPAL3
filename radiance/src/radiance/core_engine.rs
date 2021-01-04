use crate::{
    audio::AudioEngine,
    input::{InputEngine, InputEngineInternal},
    scene::{CoreScene, Director, Scene, SceneExtension},
};
use crate::{
    rendering::{self, ImguiFrame, RenderingEngine},
    scene::SceneManager,
};
use std::{cell::RefCell, rc::Rc};

pub struct CoreRadianceEngine {
    rendering_engine: Box<dyn RenderingEngine>,
    audio_engine: Box<dyn AudioEngine>,
    input_engine: Rc<RefCell<dyn InputEngineInternal>>,
    scene_manager: Box<dyn SceneManager>,
}

impl CoreRadianceEngine {
    pub(crate) fn new(
        rendering_engine: Box<dyn RenderingEngine>,
        audio_engine: Box<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngineInternal>>,
        scene_manager: Box<dyn SceneManager>,
    ) -> Self {
        Self {
            rendering_engine,
            audio_engine,
            input_engine,
            scene_manager,
        }
    }

    pub fn rendering_component_factory(&self) -> Rc<dyn rendering::ComponentFactory> {
        self.rendering_engine.component_factory()
    }

    pub fn audio_engine(&mut self) -> &mut dyn AudioEngine {
        self.audio_engine.as_mut()
    }

    pub fn input_engine(&self) -> Rc<RefCell<dyn InputEngine>> {
        self.input_engine.borrow().as_input_engine()
    }

    pub fn scene_manager(&mut self) -> &mut dyn SceneManager {
        self.scene_manager.as_mut()
    }

    pub fn update(&mut self, delta_sec: f32) {
        self.input_engine.borrow_mut().update(delta_sec);

        let scene_manager = &mut self.scene_manager;
        let ui_frame = self.rendering_engine.gui_context_mut().draw_ui(|ui| {
            scene_manager.update(ui, delta_sec);
        });

        let scene = self.scene_manager.scene_mut();
        if let Some(s) = scene {
            let extent = self.rendering_engine.view_extent();
            s.camera_mut().set_aspect(extent.0 as f32 / extent.1 as f32);
            self.rendering_engine.render(s, ui_frame);
        }
    }
}
