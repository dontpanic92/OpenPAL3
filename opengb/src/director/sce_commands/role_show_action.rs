use super::{RoleProperties, RolePropertyNames, SceneMv3Extensions};
use crate::asset_manager::AssetManager;
use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::{Mv3AnimRepeatMode, Mv3ModelEntity, ScnScene};
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, CoreScene, Entity, Scene};
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandRoleShowAction {
    role_id: String,
    action_name: String,
    repeat_mode: i32,
}

impl SceCommand for SceCommandRoleShowAction {
    fn initialize(&mut self, scene: &mut CoreScene<ScnScene>, state: &mut SceState) {
        let name = RolePropertyNames::name(&self.role_id);
        scene.entities_mut().retain(|e| e.name() != name);
        let mut entity = CoreEntity::new(
            Mv3ModelEntity::new_from_file(
                state
                    .asset_mgr()
                    .mv3_path(&self.role_id, &self.action_name)
                    .to_str()
                    .unwrap(),
                Mv3AnimRepeatMode::NoRepeat,
            ),
            &name,
        );
        entity.load();

        let position = RoleProperties::position(state, &self.role_id);
        let face_to = RoleProperties::face_to(state, &self.role_id);
        entity
            .transform_mut()
            .set_position(&position)
            .look_at(&Vec3::add(&position, &face_to));

        scene.entities_mut().push(Box::new(entity));
    }

    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        scene
            .get_mv3_entity(&RolePropertyNames::name(&self.role_id))
            .extension()
            .borrow()
            .anim_finished()
    }
}

impl SceCommandRoleShowAction {
    pub fn new(role_id: i32, action_name: String, repeat_mode: i32) -> Self {
        Self {
            role_id: format!("{}", role_id),
            action_name,
            repeat_mode,
        }
    }
}
