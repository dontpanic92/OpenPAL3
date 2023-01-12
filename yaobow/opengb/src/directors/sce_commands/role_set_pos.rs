use crate::directors::sce_vm::{SceCommand, SceState};

use crate::directors::SceneManagerExtensions;
use crate::scene::RoleController;
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandRoleSetPos {
    role_id: i32,
    nav_x: f32,
    nav_z: f32,
}

impl SceCommand for SceCommandRoleSetPos {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let position = {
            let role = scene_manager
                .get_resolved_role(state, self.role_id)
                .unwrap();
            let role_controller = RoleController::try_get_role_model(role).unwrap();
            let scene = scene_manager.core_scene_or_fail();
            scene.nav_coord_to_scene_coord(
                role_controller.get().nav_layer(),
                self.nav_x,
                self.nav_z,
            )
        };

        let role = scene_manager
            .get_resolved_role_mut(state, self.role_id)
            .unwrap();
        role.transform().borrow_mut().set_position(&position);

        state
            .global_state_mut()
            .persistent_state_mut()
            .set_position(position);

        true
    }
}

impl SceCommandRoleSetPos {
    pub fn new(role_id: i32, nav_x: i32, nav_z: i32) -> Self {
        Self {
            role_id,
            nav_x: nav_x as f32,
            nav_z: nav_z as f32,
        }
    }
}
