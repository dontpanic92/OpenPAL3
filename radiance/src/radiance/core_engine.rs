use crate::rendering::{ImguiFrame, RenderingEngine};
use crate::{
    audio::AudioEngine,
    scene::{CoreScene, Director, Scene, SceneExtension},
};

pub struct CoreRadianceEngine {
    rendering_engine: Box<dyn RenderingEngine>,
    audio_engine: Box<dyn AudioEngine>,
    scene: Option<Box<dyn Scene>>,
    director: Option<Box<dyn Director>>,
}

impl CoreRadianceEngine {
    pub fn new(
        rendering_engine: Box<dyn RenderingEngine>,
        audio_engine: Box<dyn AudioEngine>,
    ) -> Self {
        Self {
            rendering_engine,
            audio_engine,
            scene: None,
            director: None,
        }
    }

    pub fn set_director(&mut self, director: Box<dyn Director>) {
        self.director = Some(director);
    }

    pub fn load_scene<TScene: 'static + Scene>(&mut self, scene: TScene) {
        self.unload_scene();
        self.scene = Some(Box::new(scene));
        let scene_mut = self.scene.as_mut().unwrap().as_mut();
        scene_mut.load();
        self.rendering_engine.scene_loaded(scene_mut);
    }

    pub fn load_scene2<TSceneExtension: 'static + SceneExtension<TSceneExtension>>(
        &mut self,
        scene: TSceneExtension,
        fov: f32,
    ) {
        self.unload_scene();
        let extent = self.rendering_engine.view_extent();
        self.scene = Some(Box::new(CoreScene::new(
            scene,
            extent.0 as f32 / extent.1 as f32,
            fov,
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

    pub fn current_scene_mut(&mut self) -> Option<&mut Box<dyn Scene>> {
        self.scene.as_mut()
    }

    pub fn audio_engine_mut(&mut self) -> &mut dyn AudioEngine {
        self.audio_engine.as_mut()
    }

    pub fn update(&mut self, delta_sec: f32) {
        let scene = self.scene.as_mut().unwrap();

        let ui_frame = if let Some(d) = &mut self.director {
            self.rendering_engine.gui_context_mut().draw_ui(|ui| {
                d.update(scene, ui, delta_sec);
            })
        } else {
            ImguiFrame::default()
        };

        scene.update(delta_sec);
        self.rendering_engine.render(scene.as_mut(), ui_frame);
    }
}

impl Drop for CoreRadianceEngine {
    fn drop(&mut self) {
        self.unload_scene();
    }
}
