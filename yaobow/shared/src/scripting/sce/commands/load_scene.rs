use crate::{
    openpal3::{directors::SceneManagerExtensions, scene::RoleController},
    scripting::sce::{SceCommand, SceState},
};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct SceCommandLoadScene {
    name: String,
    sub_name: String,
}

impl SceCommand for SceCommandLoadScene {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let last_scene = scene_manager.scn_scene();
        let cpk_changed = last_scene
            .and_then(|s| Some(s.get().name() != &self.name))
            .or(Some(true))
            .unwrap();

        scene_manager.pop_scene();
        scene_manager.push_scene(state.asset_mgr().load_scn(&self.name, &self.sub_name));
        let e = scene_manager.get_resolved_role(state, -1).unwrap();
        let r = RoleController::get_role_controller(e.clone()).unwrap();
        r.get().set_active(true);

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
