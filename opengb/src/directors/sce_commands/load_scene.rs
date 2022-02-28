use crate::directors::sce_vm::{SceCommand, SceState};
use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::{CoreScene, SceneManager};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct SceCommandLoadScene {
    name: String,
    sub_name: String,
}

impl SceCommand for SceCommandLoadScene {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let last_scene = scene_manager.core_scene_mut();
        let cpk_changed = last_scene
            .and_then(|s| Some(s.name() != &self.name))
            .or(Some(true))
            .unwrap();

        scene_manager.pop_scene();
        scene_manager.push_scene(Box::new(CoreScene::new(
            state.asset_mgr().load_scn(&self.name, &self.sub_name),
        )));
        scene_manager
            .get_resolved_role_mut(state, -1)
            .unwrap()
            .set_active(true);

        state
            .global_state_mut()
            .persistent_state_mut()
            .set_scene_name(self.name.clone(), self.sub_name.clone());
        if cpk_changed {
            let sce = Rc::new(state.asset_mgr().load_sce(&self.name));
            state.context_mut().set_sce(sce, self.name.clone());
            state.global_state_mut().bgm_source().stop();
            state.global_state_mut().play_default_bgm();
        }

        state.try_call_proc_by_name(&format!("_{}_{}", self.name, self.sub_name));

        true
    }
}

impl SceCommandLoadScene {
    pub fn new(name: String, sub_name: String) -> Self {
        Self { name, sub_name }
    }
}
