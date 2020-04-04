use super::RoleProperties;
use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::resource_manager::ResourceManager;
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::Scene;
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandRoleSetFace {
    role_id: String,
    face_to: Vec3,
}

impl SceCommand for SceCommandRoleSetFace {
    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        RoleProperties::set_face_to(state, &self.role_id, &self.face_to);
        return true;
    }
}

impl SceCommandRoleSetFace {
    pub fn new(res_man: &Rc<ResourceManager>, role_id: i32, face_to: Vec3) -> Self {
        Self {
            role_id: format!("{}", role_id),
            face_to,
        }
    }
}
