use crosscom::ComRc;
use radiance::{
    comdef::{IApplication, IComponentImpl, IDirector},
    scene::CoreScene,
};

use crate::ComObject_EditorApplicationLoaderComponent;

pub struct EditorApplicationLoader {
    // plugin_create: Option<Box<dyn Fn(ComRc<IApplication>) -> SceneViewPlugins>>,
    app: ComRc<IApplication>,
    director: ComRc<IDirector>,
}

ComObject_EditorApplicationLoaderComponent!(super::EditorApplicationLoader);

impl IComponentImpl for EditorApplicationLoader {
    fn on_loading(&self) {
        self.app
            .engine()
            .borrow_mut()
            .imgui_context()
            .borrow_mut()
            .context_mut()
            .io_mut()
            .config_windows_move_from_title_bar_only = true;

        self.app
            .engine()
            .borrow_mut()
            .scene_manager()
            .push_scene(CoreScene::create());
        self.app
            .engine()
            .borrow_mut()
            .scene_manager()
            .set_director(self.director.clone());
    }

    fn on_updating(&self, _delta_sec: f32) {}
}

impl EditorApplicationLoader {
    pub fn new(app: ComRc<IApplication>, director: ComRc<IDirector>) -> Self {
        Self { app, director }
    }
}
