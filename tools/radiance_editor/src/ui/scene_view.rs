use imgui::Ui;
use radiance::application::Application;

use crate::application::EditorApplication;

pub struct SceneView {
    plugins: Option<SceneViewPlugins>,
}

impl SceneView {
    pub fn new(plugins: Option<SceneViewPlugins>) -> SceneView {
        Self { plugins }
    }

    pub fn initialize(&mut self, app: &mut Application<EditorApplication>) {
        if let Some(plugins) = &mut self.plugins {
            if let Some(resource_view) = &mut plugins.resource_view {
                resource_view.initialize(app);
            }
        }
    }

    pub fn render(&mut self, ui: &mut Ui, delta_sec: f32) {
        if let Some(plugins) = &mut self.plugins {
            if let Some(resource_view) = &mut plugins.resource_view {
                resource_view.render(ui, delta_sec);
            }
        }
    }
}

pub struct SceneViewPlugins {
    resource_view: Option<Box<dyn SceneViewSubView>>,
}

impl SceneViewPlugins {
    pub fn new(resource_view: Option<Box<dyn SceneViewSubView>>) -> Self {
        Self { resource_view }
    }
}

pub trait SceneViewSubView {
    fn initialize(&mut self, app: &mut Application<EditorApplication>);
    fn render(&mut self, ui: &mut Ui, delta_sec: f32);
}
