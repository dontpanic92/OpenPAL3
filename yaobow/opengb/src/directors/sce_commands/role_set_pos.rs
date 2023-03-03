use crate::directors::sce_vm::{SceCommand, SceState};

use crate::directors::SceneManagerExtensions;
use crate::scene::RoleController;
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandRoleSetPos {
    role_id: i32,
    nav_x: f32,
    nav_z: f32,
}

impl SceCommand for SceCommandRoleSetPos {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let position = {
            let role = scene_manager
                .get_resolved_role(state, self.role_id)
                .unwrap();
            let role_controller = RoleController::get_role_controller(role).unwrap();
            let scene = scene_manager.scn_scene().unwrap().get();
            scene.nav_coord_to_scene_coord(
                role_controller.get().nav_layer(),
                self.nav_x,
                self.nav_z,
            )
        };

        let role = scene_manager
            .get_resolved_role(state, self.role_id)
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
