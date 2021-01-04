use super::SceneManager;
use imgui::Ui;
use std::{cell::RefCell, rc::Rc};

pub trait Director {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>>;
}
