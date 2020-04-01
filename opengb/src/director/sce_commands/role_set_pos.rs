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
pub struct SceCommandRoleSetPos {
    role_id: String,
    position: Vec3,
}

impl SceCommand for SceCommandRoleSetPos {
    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
        ui: &mut Ui,
        state: &mut HashMap<String, Box<dyn Any>>,
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
