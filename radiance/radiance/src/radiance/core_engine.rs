use mini_fs::MiniFs;

use super::DebugLayer;
use crate::{
    audio::AudioEngine,
    imgui::ImguiContext,
    input::{InputEngine, InputEngineInternal},
};
use crate::{
    rendering::{self, RenderingEngine},
    scene::SceneManager,
};
use std::{cell::RefCell, rc::Rc};

pub struct CoreRadianceEngine {
    rendering_engine: Rc<RefCell<dyn RenderingEngine>>,
    audio_engine: Rc<dyn AudioEngine>,
    input_engine: Rc<RefCell<dyn InputEngineInternal>>,
    imgui_context: Rc<RefCell<ImguiContext>>,
    scene_manager: Option<Box<dyn SceneManager>>,
    virtual_fs: Rc<RefCell<MiniFs>>,
    debug_layer: Option<Box<dyn DebugLayer>>,
}

impl CoreRadianceEngine {
    pub(crate) fn new(
        rendering_engine: Rc<RefCell<dyn RenderingEngine>>,
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngineInternal>>,
        imgui_context: Rc<RefCell<ImguiContext>>,
        scene_manager: Box<dyn SceneManager>,
    ) -> Self {
        Self {
            rendering_engine,
            audio_engine,
            input_engine,
            imgui_context,
            scene_manager: Some(scene_manager),
            virtual_fs: Rc::new(RefCell::new(MiniFs::new(false))),
            debug_layer: None,
        }
    }

    pub fn rendering_component_factory(&self) -> Rc<dyn rendering::ComponentFactory> {
        self.rendering_engine.borrow().component_factory()
    }

    pub fn audio_engine(&mut self) -> Rc<dyn AudioEngine> {
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

    pub fn scene_manager(&mut self) -> &mut dyn SceneManager {
        self.scene_manager.as_mut().unwrap().as_mut()
    }

    pub fn update(&mut self, delta_sec: f32) {
        self.input_engine.borrow_mut().update(delta_sec);

        let scene_manager = self.scene_manager.as_mut().unwrap();
        let debug_layer = self.debug_layer.as_mut();
        let ui_frame = self.imgui_context.borrow_mut().draw_ui(delta_sec, |ui| {
            scene_manager.update(ui, delta_sec);
            if let Some(dl) = debug_layer {
                dl.update(scene_manager.as_mut(), ui, delta_sec);
            }
        });

        use crate::{math::Rect, scene::Viewport};
        let scene = self.scene_manager.as_mut().unwrap().scene();
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

        /*
        TODO: how about multiple scenes?
        */
    }
}

impl Drop for CoreRadianceEngine {
    fn drop(&mut self) {
        self.scene_manager = None;
    }
}
