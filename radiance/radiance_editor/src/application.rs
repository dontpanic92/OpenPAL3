use crosscom::ComRc;
use radiance::{
    comdef::{IApplication, IComponentImpl},
    scene::CoreScene,
};

use crate::{
    director::UiDirector, ui::scene_view::SceneViewPlugins,
    ComObject_EditorApplicationLoaderComponent,
};

pub struct EditorApplicationLoader {
    // plugin_create: Option<Box<dyn Fn(ComRc<IApplication>) -> SceneViewPlugins>>,
    app: ComRc<IApplication>,
    plugins: Option<SceneViewPlugins>,
}

ComObject_EditorApplicationLoaderComponent!(super::EditorApplicationLoader);

impl IComponentImpl for EditorApplicationLoader {
    fn on_loading(&self) {
        let input = self.app.engine().borrow().input_engine();
        let director = UiDirector::new(self.plugins.clone(), input);

        self.app
            .engine()
            .borrow_mut()
            .scene_manager()
            .push_scene(CoreScene::create());
        self.app
            .engine()
            .borrow_mut()
            .scene_manager()
            .set_director(director);
    }

    fn on_updating(&self, _delta_sec: f32) {}
}

impl EditorApplicationLoader {
    pub fn new(app: ComRc<IApplication>, plugins: Option<SceneViewPlugins>) -> Self {
        Self { app, plugins }
    }
}
