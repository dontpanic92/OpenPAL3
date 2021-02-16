use crate::directors::sce_director::{SceCommand, SceState};

use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::Entity;
use radiance::{math::Vec3, scene::SceneManager};

#[derive(Clone)]
pub struct SceCommandRoleTurnFace {
    role_id: i32,
    degree: f32,
}

impl SceCommand for SceCommandRoleTurnFace {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        scene_manager
            .core_scene_mut_or_fail()
            .get_role_entity_mut(self.role_id)
            .transform_mut()
            .rotate_axis_angle_local(&Vec3::UP, -self.degree.to_radians());
        true
    }
}

impl SceCommandRoleTurnFace {
    pub fn new(role_id: i32, degree: f32) -> Self {
        Self { role_id, degree }
    }
}
