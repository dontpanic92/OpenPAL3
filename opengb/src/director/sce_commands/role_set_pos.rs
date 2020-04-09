use super::{nav_coord_to_scene_coord, RoleProperties};
use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::resource_manager::ResourceManager;
use crate::scene::ScnScene;
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::CoreScene;
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandRoleSetPos {
    role_id: String,
    position: Vec3,
}

impl SceCommand for SceCommandRoleSetPos {
    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let position = nav_coord_to_scene_coord(scene, &self.position);
        RoleProperties::set_position(state, &self.role_id, &position);
        return true;
    }
}

impl SceCommandRoleSetPos {
    pub fn new(res_man: &Rc<ResourceManager>, role_id: i32, position: Vec3) -> Self {
        Self {
            role_id: format!("{}", role_id),
            position,
        }
    }
}
