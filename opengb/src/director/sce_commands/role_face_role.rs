use super::RolePropertyNames;
use super::{RoleProperties, SceneMv3Extensions};
use crate::director::sce_director::SceCommand;
use crate::resource_manager::ResourceManager;
use crate::scene::{Mv3AnimRepeatMode, Mv3ModelEntity};
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, Entity, Scene};
use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandRoleFaceRole {
    role_id: String,
    role_id2: String,
}

impl SceCommand for SceCommandRoleFaceRole {
    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
        ui: &mut Ui,
        state: &mut HashMap<String, Box<dyn Any>>,
        delta_sec: f32,
    ) -> bool {
        let position = RoleProperties::position(state, &self.role_id2);
        RoleProperties::set_face_to(state, &self.role_id, &position);

        let entity = scene.get_mv3_entity(&RolePropertyNames::name(&self.role_id));
        entity
            .transform_mut()
            .look_at(&position)
            .rotate_axis_angle_local(&Vec3::UP, 180_f32.to_radians());
        return true;
    }
}

impl SceCommandRoleFaceRole {
    pub fn new(res_man: &Rc<ResourceManager>, role_id: i32, role_id2: i32) -> Self {
        Self {
            role_id: format!("{}", role_id),
            role_id2: format!("{}", role_id2),
        }
    }
}
