#![feature(arbitrary_self_types)]
#![feature(drain_filter)]
mod directors;
mod exporters;

use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

use directors::DevToolsDirector;
use imgui::Ui;
use opengb::{asset_manager::AssetManager, config::OpenGbConfig};
use radiance::application::Application;
use radiance::scene::{Director, SceneManager};
use radiance_editor::application::EditorApplication;
use radiance_editor::core::IViewContentImpl;
use radiance_editor::ui::scene_view::SceneViewPlugins;

const TITLE: &str = "妖弓编辑器 - OpenPAL3";

struct SceneViewResourceView {
    ui: RefCell<Option<Rc<RefCell<DevToolsDirector>>>>,
}

radiance_editor::ComObject_ResourceViewContent!(crate::SceneViewResourceView);

impl IViewContentImpl for SceneViewResourceView {
    fn render(&self, scene_manager: i64, ui: i64, delta_sec: f32) {
        // CrossCom hack
        let scene_manager = unsafe {
            (scene_manager as *mut c_void as *mut radiance::scene::DefaultSceneManager)
                .as_mut()
                .unwrap()
        };

        let ui = unsafe { (ui as *const c_void as *mut Ui).as_mut().unwrap() };

        let mut director = self.ui.borrow_mut();
        let view = director.as_mut().unwrap();
        view.borrow_mut().update(scene_manager, ui, delta_sec);
    }
}

impl SceneViewResourceView {
    pub fn new(config: OpenGbConfig, app: &mut Application<EditorApplication>) -> Self {
        app.set_title(TITLE);

        let factory = app.engine_mut().rendering_component_factory();
        let asset_mgr = AssetManager::new(factory, &config.asset_path);
        let audio_engine = app.engine_mut().audio_engine();
        let ui = Some(DevToolsDirector::new(audio_engine, Rc::new(asset_mgr)));

        SceneViewResourceView {
            ui: RefCell::new(ui),
        }
    }
}

fn main() {
    let mut application = EditorApplication::new_with_plugin(|app| {
        let config = OpenGbConfig::load("openpal3.toml", "OPENPAL3");
        let resource_view_content = SceneViewResourceView::new(config, app);

        SceneViewPlugins::new(Some(crosscom::ComRc::from_object(resource_view_content)))
    });
    application.initialize();
    application.run();
}
