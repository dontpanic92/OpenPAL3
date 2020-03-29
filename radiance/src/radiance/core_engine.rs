use crate::rendering::RenderingEngine;
use crate::scene::{CoreScene, Scene, SceneExtension};

pub struct CoreRadianceEngine<TRenderingEngine: RenderingEngine> {
    rendering_engine: TRenderingEngine,
    scene: Option<Box<dyn Scene>>,
}

impl<TRenderingEngine: RenderingEngine> CoreRadianceEngine<TRenderingEngine> {
    pub fn new(rendering_engine: TRenderingEngine) -> Self {
        Self {
            rendering_engine,
            scene: None,
        }
    }

    pub fn load_scene<TScene: 'static + Scene>(&mut self, scene: TScene) {
        self.unload_scene();
        self.scene = Some(Box::new(scene));
        let scene_mut = self.scene.as_mut().unwrap().as_mut();
        scene_mut.load();
        self.rendering_engine.scene_loaded(scene_mut);
    }

    pub fn load_scene2<TSceneExtension: 'static + SceneExtension<TSceneExtension>>(&mut self, scene: TSceneExtension) {
        self.unload_scene();
        let extent = self.rendering_engine.view_extent();
        self.scene = Some(Box::new(CoreScene::new(
            scene,
            extent.0 as f32 / extent.1 as f32,
        )));
        let scene_mut = self.scene.as_mut().unwrap().as_mut();
        scene_mut.load();
        self.rendering_engine.scene_loaded(scene_mut);
    }

    pub fn unload_scene(&mut self) {
        match self.scene.as_mut() {
            Some(s) => s.unload(),
            None => (),
        }

        self.scene = None;
    }

    pub fn update(&mut self, delta_sec: f32) {
        self.scene.as_mut().unwrap().update(delta_sec);
        self.rendering_engine
            .render(self.scene.as_mut().unwrap().as_mut());
    }
}

impl<TRenderingEngine: RenderingEngine> Drop for CoreRadianceEngine<TRenderingEngine> {
    fn drop(&mut self) {
        self.unload_scene();
    }
}
