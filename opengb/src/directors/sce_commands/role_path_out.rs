use crate::directors::sce_vm::{SceCommand, SceState};

use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::SceneManager;

use super::SceCommandRolePathTo;

#[derive(Debug, Clone)]
pub struct SceCommandRolePathOut {
    role_id: i32,
    cmd_path_to: SceCommandRolePathTo,
}

impl SceCommand for SceCommandRolePathOut {
    fn initialize(&mut self, scene_manager: &mut dyn SceneManager, state: &mut SceState) {
        self.cmd_path_to.initialize(scene_manager, state);
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        if self.cmd_path_to.update(scene_manager, ui, state, delta_sec) {
            scene_manager.resolve_role_mut_do(state, self.role_id, |r| r.set_active(false));
            true
        } else {
            false
        }
    }
}

impl SceCommandRolePathOut {
    pub fn new(role_id: i32, nav_x: i32, nav_z: i32, unknown: i32) -> Self {
        Self {
            role_id,
            cmd_path_to: SceCommandRolePathTo::new(role_id, nav_x, nav_z, unknown),
        }
    }
}
