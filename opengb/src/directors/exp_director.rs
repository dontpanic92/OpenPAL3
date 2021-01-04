use std::{cell::RefCell, rc::Rc};

use radiance::scene::{Director, SceneManager};

use super::SceDirector;

pub struct ExplorationDirector {
    sce_director: Rc<RefCell<SceDirector>>,
}

impl ExplorationDirector {
    pub fn new(sce_director: Rc<RefCell<SceDirector>>) -> Self {
        Self { sce_director }
    }
}

impl Director for ExplorationDirector {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut imgui::Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        None
    }
}
