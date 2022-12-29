use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use imgui::Ui;
use radiance::{input::InputEngine, scene::SceneManager};

use crate::core::IViewContent;

use self::{
    node_view::NodeView, property_view::PropertyView, resource_view::ResourceView,
    scene_edit_view::SceneEditView,
};

mod node_view;
mod property_view;
mod resource_view;
mod scene_edit_view;

pub struct SceneView {
    node_view: NodeView,
    scene_edit_view: SceneEditView,
    property_view: PropertyView,
    resource_view: ResourceView,
}

impl SceneView {
    pub fn new(
        input: Rc<RefCell<dyn InputEngine>>,
        mut plugins: Option<SceneViewPlugins>,
    ) -> SceneView {
        let resource_view_content = plugins
            .as_mut()
            .and_then(|p| p.resource_view_content.take());
        Self {
            node_view: NodeView {},
            property_view: PropertyView {},
            scene_edit_view: SceneEditView::new(input),
            resource_view: ResourceView::new(resource_view_content),
        }
    }

    pub fn render(&mut self, scene_manager: &mut dyn SceneManager, ui: &Ui, delta_sec: f32) {
        self.node_view.render(scene_manager, ui, delta_sec);
        self.property_view.render(scene_manager, ui, delta_sec);
        self.scene_edit_view.render(scene_manager, ui, delta_sec);
        self.resource_view.render(scene_manager, ui, delta_sec);
    }
}

pub struct SceneViewPlugins {
    resource_view_content: Option<ComRc<IViewContent>>,
}

impl SceneViewPlugins {
    pub fn new(resource_view_content: Option<ComRc<IViewContent>>) -> Self {
        Self {
            resource_view_content,
        }
    }
}
