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
    painted: bool,
    started: bool,
    cpk_changed: bool,
}

impl SceCommand for SceCommandLoadScene {
    fn blocks_script(&self) -> bool {
        true
    }

    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let staged = state.asset_mgr().game() == crate::GameType::PAL3;
        if staged && !self.painted {
            self.painted = true;
            state.render_loading(0., 0.);
            return false;
        }
        if !self.started {
            let last_scene = scene_manager.scn_scene();
            if let Some(scene) = &last_scene {
                scene
                    .inner::<crate::openpal3::scene::ScnScene>()
                    .snapshot_mechanisms(&mut state.global_state_mut().persistent_state_mut());
            }
            self.cpk_changed = last_scene
                .and_then(|s| {
                    Some(s.inner::<crate::openpal3::scene::ScnScene>().name() != &self.name)
                })
                .or(Some(true))
                .unwrap();

            scene_manager.pop_scene();
            let scene = if staged {
                state
                    .asset_mgr()
                    .load_scn_deferred(&self.name, &self.sub_name)
            } else {
                state.asset_mgr().load_scn(&self.name, &self.sub_name)
            };
            scene_manager.push_scene(scene);
            self.started = true;
            if staged {
                state.render_loading(0., 0.);
                return false;
            }
        }
        if staged {
            let progress = scene_manager
                .scn_scene()
                .unwrap()
                .inner::<crate::openpal3::scene::ScnScene>()
                .advance_loading();
            state.render_loading(progress, delta_sec);
            if progress < 1. {
                return false;
            }
        }
        if let Some(scene) = scene_manager.scn_scene() {
            scene
                .inner::<crate::openpal3::scene::ScnScene>()
                .restore_mechanisms(&state.global_state().persistent_state());
        }
        let e = scene_manager.get_resolved_role(state, -1).unwrap();
        let r = RoleController::get_role_controller(e.clone()).unwrap();
        r.inner::<RoleController>().set_active(true);

        state
            .global_state_mut()
            .persistent_state_mut()
            .set_scene_name(self.name.clone(), self.sub_name.clone());
        if self.cpk_changed {
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
        Self {
            name,
            sub_name,
            painted: false,
            started: false,
            cpk_changed: false,
        }
    }
}
