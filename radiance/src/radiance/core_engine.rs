use super::RadianceEngine;
use crate::rendering::RenderingEngine;
use crate::scene::Scene;

pub struct CoreRadianceEngine<TRenderingEngine: RenderingEngine> {
    rendering_engine: TRenderingEngine,
    scene: Option<Scene>,
}

impl<TRenderingEngine: RenderingEngine> RadianceEngine for CoreRadianceEngine<TRenderingEngine> {
    fn update(&mut self) {
        self.rendering_engine.render();
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
