use super::RoleProperties;
use super::RolePropertyNames;
use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::resource_manager::ResourceManager;
use crate::scene::{Mv3AnimRepeatMode, Mv3ModelEntity, ScnScene};
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, CoreScene, Entity, Scene};
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandRoleActive {
    res_man: Rc<ResourceManager>,
    role_id: String,
    position: Vec3,
}

impl SceCommand for SceCommandRoleActive {
    fn initialize(&mut self, scene: &mut CoreScene<ScnScene>, state: &mut SceState) {
        scene
            .entities_mut()
            .retain(|e| e.name() != RolePropertyNames::name(&self.role_id));

        RoleProperties::set_position(state, &self.role_id, &self.position);
    }

    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let mut entity = CoreEntity::new(
            Mv3ModelEntity::new_from_file(
                &self.res_man.mv3_path("101", "C11").to_str().unwrap(),
                Mv3AnimRepeatMode::Repeat,
            ),
            &RolePropertyNames::name(&self.role_id),
        );
        entity.load();
        entity.transform_mut().set_position(&self.position);

        scene.entities_mut().push(Box::new(entity));

        return true;
    }
}

impl SceCommandRoleActive {
    pub fn new(res_man: &Rc<ResourceManager>, role_id: i32, position: Vec3) -> Self {
        Self {
            res_man: res_man.clone(),
            role_id: format!("{}", role_id),
            position,
        }
    }
}
