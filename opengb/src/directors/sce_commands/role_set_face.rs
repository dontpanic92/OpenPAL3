use crate::directors::sce_director::{SceCommand, SceState};

use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::Entity;
use radiance::{math::Vec3, scene::SceneManager};

#[derive(Clone)]
pub struct SceCommandRoleSetFace {
    role_id: i32,
    face_to: Vec3,
}

impl SceCommand for SceCommandRoleSetFace {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let scene = scene_manager.core_scene_mut_or_fail();
        let entity = scene.get_role_entity_mut(self.role_id);
        let position = entity.transform().position();
        let target = Vec3::add(&position, &self.face_to);

        entity.transform_mut().look_at(&target);
        return true;
    }
}

impl SceCommandRoleSetFace {
    pub fn new(role_id: i32, direction: i32) -> Self {
        let face_to = match direction {
            0 => super::Direction::SOUTH,
            1 => super::Direction::SOUTHEAST,
            2 => super::Direction::EAST,
            3 => super::Direction::NORTHEAST,
            4 => super::Direction::NORTH,
            5 => super::Direction::NORTHWEST,
            6 => super::Direction::WEST,
            7 => super::Direction::SOUTHWEST,
            _ => unreachable!(),
        };

        Self { role_id, face_to }
    }
}
