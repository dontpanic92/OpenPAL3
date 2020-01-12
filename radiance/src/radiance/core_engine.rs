use super::RadianceEngine;
use crate::rendering::RenderingEngine;
use crate::scene::Scene;

pub struct CoreRadianceEngine<TRenderingEngine: RenderingEngine> {
    rendering_engine: TRenderingEngine,
    scene: Option<Scene>,
}

impl<TRenderingEngine: RenderingEngine> RadianceEngine for CoreRadianceEngine<TRenderingEngine> {
    fn load_scene(&mut self) {
        self.scene = Some(Scene::new());
        self.rendering_engine
            .scene_loaded(self.scene.as_mut().unwrap());
    }

    fn unload_scene(&mut self) {
        self.scene = None;
    }

    fn update(&mut self) {
        self.rendering_engine.render(self.scene.as_mut().unwrap());
    }
}

impl<TRenderingEngine: RenderingEngine> CoreRadianceEngine<TRenderingEngine> {
    pub fn new(rendering_engine: TRenderingEngine) -> Self {
        Self {
            rendering_engine,
            scene: None,
        }
    }
}

impl<TRenderingEngine: RenderingEngine> Drop for CoreRadianceEngine<TRenderingEngine> {
    fn drop(&mut self) {
        self.unload_scene();
    }
}
