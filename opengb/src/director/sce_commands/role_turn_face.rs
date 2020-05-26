use super::{map_role_id, SceneRoleExtensions};
use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::ScnScene;
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::{CoreScene, Entity};

#[derive(Clone)]
pub struct SceCommandRoleTurnFace {
    role_id: String,
    degree: f32,
}

impl SceCommand for SceCommandRoleTurnFace {
    fn initialize(&mut self, scene: &mut CoreScene<ScnScene>, state: &mut SceState) {}

    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        scene
            .get_role_entity(&self.role_id)
            .transform_mut()
            .rotate_axis_angle_local(&Vec3::UP, -self.degree.to_radians());
        true
    }
}

impl SceCommandRoleTurnFace {
    pub fn new(role_id: i32, degree: f32) -> Self {
        Self {
            role_id: map_role_id(role_id).to_string(),
            degree,
        }
    }
}
