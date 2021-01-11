use std::rc::Rc;

use crate::directors::sce_director::{SceCommand, SceState};
use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::{CoreScene, SceneManager};

#[derive(Clone)]
pub struct SceCommandLoadScene {
    name: String,
    sub_name: String,
}

impl SceCommand for SceCommandLoadScene {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let last_scene = scene_manager.core_scene_mut();
        let cpk_changed = last_scene
            .and_then(|s| Some(s.name() == &self.name))
            .or(Some(true))
            .unwrap();

        scene_manager.pop_scene();
        scene_manager.push_scene(Box::new(CoreScene::new(
            state.asset_mgr().load_scn(&self.name, &self.sub_name),
        )));

        if cpk_changed {
            let sce = Rc::new(state.asset_mgr().load_sce(&self.name));
            state.vm_context_mut().set_sce(sce);
        }

        true
    }
}

impl SceCommandLoadScene {
    pub fn new(name: String, sub_name: String) -> Self {
        Self { name, sub_name }
    }
}
