use radiance::application::{Application, ApplicationExtension};

use crate::{director::UiDirector, scene::EditorScene, ui::scene_view::SceneViewPlugins};

pub struct EditorApplication {
    plugin_create: Option<Box<dyn Fn(&mut Application<EditorApplication>) -> SceneViewPlugins>>,
}

impl ApplicationExtension<EditorApplication> for EditorApplication {
    fn on_initialized(&mut self, app: &mut Application<EditorApplication>) {
        let scene_view_plugins = self.plugin_create.as_ref().map(|c| c(app));

        let logger = simple_logger::SimpleLogger::new();

        // workaround panic on Linux for 'Could not determine the UTC offset on this system'
        // see: https://github.com/borntyping/rust-simple_logger/issues/47
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
        let logger = logger.with_utc_timestamps();

        logger.init().unwrap();

        let input = app.engine_mut().input_engine();
        let director = UiDirector::new(scene_view_plugins, input);

        app.engine_mut()
            .scene_manager()
            .push_scene(Box::new(EditorScene::new()));
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
