use super::RoleProperties;
use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::ScnScene;
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::CoreScene;

#[derive(Clone)]
pub struct SceCommandRoleSetFace {
    role_id: String,
    face_to: Vec3,
}

impl SceCommand for SceCommandRoleSetFace {
    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        RoleProperties::set_face_to(state, &self.role_id, &self.face_to);
        return true;
    }
}

impl SceCommandRoleSetFace {
    pub fn new(role_id: i32, direction: i32) -> Self {
        let face_to = match direction {
            0 => super::Direction::EAST,
            1 => super::Direction::WEST,
            2 => super::Direction::NORTH,
            3 => super::Direction::SOUTH,
            _ => unreachable!(),
        };
        Self {
            role_id: format!("{}", role_id),
            face_to,
        }
    }
}
