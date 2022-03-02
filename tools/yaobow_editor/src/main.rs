#![feature(arbitrary_self_types)]
#![feature(drain_filter)]
mod directors;

use std::cell::RefCell;
use std::rc::Rc;

use directors::DevToolsDirector;
use imgui::Ui;
use opengb::{asset_manager::AssetManager, config::OpenGbConfig};
use radiance::application::Application;
use radiance::scene::{Director, SceneManager};
use radiance_editor::application::EditorApplication;
use radiance_editor::ui::scene_view::{SceneViewPlugins, SceneViewSubView};

struct SceneViewResourceView {
    ui: Option<Rc<RefCell<DevToolsDirector>>>,
}

impl SceneViewSubView for SceneViewResourceView {
    fn render(&mut self, scene_manager: &mut dyn SceneManager, ui: &Ui, delta_sec: f32) {
        let view = self.ui.as_mut().unwrap();
        view.borrow_mut().update(scene_manager, ui, delta_sec);
    }
}

impl SceneViewResourceView {
    pub fn new(config: OpenGbConfig, app: &mut Application<EditorApplication>) -> Self {
        app.set_title("妖弓编辑器 - OpenPAL3");

        let factory = app.engine_mut().rendering_component_factory();
        let asset_mgr = AssetManager::new(factory, &config.asset_path);
        let input_engine = app.engine_mut().input_engine();
        let audio_engine = app.engine_mut().audio_engine();
        let ui = Some(DevToolsDirector::new(
            input_engine,
            audio_engine,
            Rc::new(asset_mgr),
        ));

        SceneViewResourceView { ui }
    }
}

fn main() {
    let mut application = EditorApplication::new_with_plugin(|app| {
        let config = OpenGbConfig::load("openpal3.toml", "OPENPAL3");
        let resource_view_content = SceneViewResourceView::new(config, app);

        SceneViewPlugins::new(Some(Box::new(resource_view_content)))
    });
    application.initialize();
    application.run();
}
