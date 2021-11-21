use crate::directors::sce_vm::{SceCommand, SceState};

use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::Entity;
use radiance::{math::Vec3, scene::SceneManager};

#[derive(Debug, Clone)]
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
        scene_manager.resolve_role_mut_do(state, self.role_id, |entity| {
            let position = entity.transform().position();
            let target = Vec3::add(&position, &self.face_to);

            entity.transform_mut().look_at(&target);
        });
        true
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
            i => {
                log::warn!("Unrecognized face_to parameter: {}", i);
                super::Direction::SOUTH
            }
        };

        Self { role_id, face_to }
    }
}
