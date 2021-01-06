use super::map_role_id;
use crate::directors::sce_director::SceCommand;
use crate::directors::sce_state::SceState;
use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::{Entity, SceneManager};

#[derive(Clone)]
pub struct SceCommandRoleSetPos {
    role_id: String,
    nav_x: f32,
    nav_z: f32,
}

impl SceCommand for SceCommandRoleSetPos {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let scene = scene_manager.scene_mut_or_fail();
        let position = scene.nav_coord_to_scene_coord(self.nav_x, self.nav_z);
        scene
            .get_role_entity_mut(&self.role_id)
            .transform_mut()
            .set_position(&position);
        return true;
    }
}

impl SceCommandRoleSetPos {
    pub fn new(role_id: i32, nav_x: i32, nav_z: i32) -> Self {
        Self {
            role_id: map_role_id(role_id).to_string(),
            nav_x: nav_x as f32,
            nav_z: nav_z as f32,
        }
    }
}
