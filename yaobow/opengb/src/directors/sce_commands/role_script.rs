use crate::directors::sce_vm::{SceCommand, SceState};

use crate::directors::SceneManagerExtensions;
use crate::scene::RoleController;
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandRoleScript {
    role_id: i32,
    proc_id: i32,
}

impl SceCommand for SceCommandRoleScript {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let role = scene_manager
            .get_resolved_role(state, self.role_id)
            .unwrap();

        let role_controller = RoleController::get_role_controller(role).unwrap();
        role_controller.get().set_proc_id(self.proc_id);
        true
    }
}

impl SceCommandRoleScript {
    pub fn new(role_id: i32, proc_id: i32) -> Self {
        Self { role_id, proc_id }
    }
}
