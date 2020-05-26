use super::{map_role_id, SceneRoleExtensions};
use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::ScnScene;
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::{CoreScene, Entity};

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
        let entity = scene.get_role_entity(&self.role_id);
        let position = entity.transform().position();
        let target = Vec3::add(&position, &self.face_to);

        entity.transform_mut().look_at(&target);
        return true;
    }
}

impl SceCommandRoleSetFace {
    pub fn new(role_id: i32, direction: i32) -> Self {
        let face_to = match direction {
            0 => super::Direction::NORTH,
            1 => super::Direction::NORTHEAST,
            2 => super::Direction::EAST,
            3 => super::Direction::SOUTHEAST,
            4 => super::Direction::SOUTH,
            5 => super::Direction::SOUTHWEST,
            6 => super::Direction::WEST,
            7 => super::Direction::NORTHWEST,
            _ => unreachable!(),
        };

        Self {
            role_id: map_role_id(role_id).to_string(),
            face_to,
        }
    }
}
