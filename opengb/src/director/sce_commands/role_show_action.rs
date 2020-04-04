use super::{RoleProperties, RolePropertyNames, SceneMv3Extensions};
use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::resource_manager::ResourceManager;
use crate::scene::{Mv3AnimRepeatMode, Mv3ModelEntity};
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, Entity, Scene};
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandRoleShowAction {
    res_man: Rc<ResourceManager>,
    role_id: String,
    action_name: String,
    repeat_mode: i32,
}

impl SceCommand for SceCommandRoleShowAction {
    fn initialize(&mut self, scene: &mut Box<dyn Scene>, state: &mut SceState) {
        let name = RolePropertyNames::name(&self.role_id);
        scene.entities_mut().retain(|e| e.name() != name);
        let mut entity = CoreEntity::new(
            Mv3ModelEntity::new_from_file(
                &self
                    .res_man
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
            .look_at(&Vec3::add(&position, &face_to))
            .rotate_axis_angle_local(&Vec3::UP, 180_f32.to_radians());

        scene.entities_mut().push(Box::new(entity));
    }

    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
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
    pub fn new(
        res_man: &Rc<ResourceManager>,
        role_id: i32,
        action_name: &str,
        repeat_mode: i32,
    ) -> Self {
        Self {
            res_man: res_man.clone(),
            role_id: format!("{}", role_id),
            action_name: action_name.to_owned(),
            repeat_mode,
        }
    }
}
