use crate::directors::sce_vm::{SceCommand, SceState};

use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Clone)]
pub struct SceCommandRoleScript {
    role_id: i32,
    proc_id: i32,
}

impl SceCommand for SceCommandRoleScript {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        scene_manager
            .get_resolved_role_mut(state, self.role_id)
            .unwrap()
            .set_proc_id(self.proc_id);
        true
    }
}

impl SceCommandRoleScript {
    pub fn new(role_id: i32, proc_id: i32) -> Self {
        Self { role_id, proc_id }
    }
}
