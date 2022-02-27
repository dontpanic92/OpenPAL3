use radiance::application::{Application, ApplicationExtension};

use crate::{director::UiDirector, scene::EditorScene, ui::scene_view::SceneViewPlugins};

pub struct EditorApplication {
    create_info: Option<EditorApplicationCreateInfo>,
}

impl ApplicationExtension<EditorApplication> for EditorApplication {
    fn on_initialized(&mut self, app: &mut Application<EditorApplication>) {
        let mut create_info = self.create_info.take().unwrap();
        let scene_view_plugins = create_info.scene_view_plugins.take();

        let logger = simple_logger::SimpleLogger::new();
        logger.init().unwrap();

        let director = UiDirector::new(app, scene_view_plugins);

        app.engine_mut()
            .scene_manager()
            .push_scene(Box::new(EditorScene::new()));
        app.engine_mut()
            .scene_manager()
            .set_director(director);
    }

    fn on_updating(&mut self, _app: &mut Application<EditorApplication>, _delta_sec: f32) {}
}

impl EditorApplication {
    pub fn new(scene_view_plugins: Option<SceneViewPlugins>) -> Application<Self> {
        Application::new(EditorApplication {
            create_info: Some(EditorApplicationCreateInfo { scene_view_plugins }),
        })
    }
}

struct EditorApplicationCreateInfo {
    scene_view_plugins: Option<SceneViewPlugins>,
}
