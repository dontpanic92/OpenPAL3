use super::RoleProperties;
use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::resource_manager::ResourceManager;
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::Scene;
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandRoleSetPos {
    role_id: String,
    position: Vec3,
}

impl SceCommand for SceCommandRoleSetPos {
    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        RoleProperties::set_position(state, &self.role_id, &self.position);
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
