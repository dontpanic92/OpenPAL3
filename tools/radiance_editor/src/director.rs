use imgui::Ui;
use radiance::{
    scene::{Director, SceneManager}, application::Application,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use crate::{ui::scene_view::{SceneView, SceneViewPlugins, SceneViewSubView}, application::EditorApplication};

pub struct UiDirector {
    shared_self: Weak<RefCell<Self>>,
    scene_view: SceneView,
}

impl UiDirector {
    pub fn new(
        app: &mut Application<EditorApplication>,
        scene_view_plugins: Option<SceneViewPlugins>,
    ) -> Rc<RefCell<Self>> {
        let mut _self = Rc::new(RefCell::new(Self {
            shared_self: Weak::new(),
            scene_view: SceneView::new(scene_view_plugins),
        }));

        _self.borrow_mut().shared_self = Rc::downgrade(&_self);
        _self.borrow_mut().scene_view.initialize(app);

        _self
    }
}

impl Director for UiDirector {
    fn activate(&mut self, _scene_manager: &mut dyn SceneManager) {}

    fn update(
        &mut self,
        _scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        self.scene_view.render(ui, delta_sec);
        None
    }
}
