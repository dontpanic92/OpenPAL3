use radiance::{
    application::{Application, ApplicationExtension},
    scene::CoreScene,
};

use crate::{director::UiDirector, ui::scene_view::SceneViewPlugins};

pub struct EditorApplication {
    plugin_create: Option<Box<dyn Fn(&mut Application<EditorApplication>) -> SceneViewPlugins>>,
}

impl ApplicationExtension<EditorApplication> for EditorApplication {
    fn on_initialized(&mut self, app: &mut Application<EditorApplication>) {
        let scene_view_plugins = self.plugin_create.as_ref().map(|c| c(app));

        let input = app.engine_mut().input_engine();
        let director = UiDirector::new(scene_view_plugins, input);

        app.engine_mut()
            .scene_manager()
            .push_scene(CoreScene::create());
        app.engine_mut().scene_manager().set_director(director);
    }

    fn on_updating(&mut self, _app: &mut Application<EditorApplication>, _delta_sec: f32) {}
}

impl EditorApplication {
    pub fn new() -> Application<Self> {
        Application::new(EditorApplication {
            plugin_create: None,
        })
    }

    pub fn new_with_plugin<
        T: 'static + Fn(&mut Application<EditorApplication>) -> SceneViewPlugins,
    >(
        plugin_create: T,
    ) -> Application<Self> {
        Application::new(EditorApplication {
            plugin_create: Some(Box::new(plugin_create)),
        })
    }
}
