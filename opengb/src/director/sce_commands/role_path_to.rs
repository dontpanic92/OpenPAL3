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
pub struct SceCommandRolePathTo {
    res_man: Rc<ResourceManager>,
    role_id: String,
    from: Vec3,
    to: Vec3,
}

impl SceCommand for SceCommandRolePathTo {
    fn initialize(&mut self, scene: &mut Box<dyn Scene>, state: &mut SceState) {
        let name = RolePropertyNames::name(&self.role_id);
        scene.entities_mut().retain(|e| e.name() != name);
        let mut entity = CoreEntity::new(
            Mv3ModelEntity::new_from_file(
                &self
                    .res_man
                    .mv3_path(&self.role_id, "c02")
                    .to_str()
                    .unwrap(),
                Mv3AnimRepeatMode::Repeat,
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
        scene: &mut Box<dyn Scene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        const SPEED: f32 = 50.;

        let position = RoleProperties::position(state, &self.role_id);
        let step = SPEED * delta_sec;
        let remain = Vec3::sub(&self.to, &position);
        let completed = remain.norm() < step;
        let new_position = if completed {
            self.to
        } else {
            Vec3::add(&position, &Vec3::dot(step, &Vec3::normalized(&remain)))
        };

        let entity = scene.get_mv3_entity(&RolePropertyNames::name(&self.role_id));
        entity
            .transform_mut()
            .look_at(&self.to)
            .rotate_axis_angle_local(&Vec3::UP, 180_f32.to_radians())
            .set_position(&new_position);
        RoleProperties::set_position(state, &self.role_id, &new_position);
        RoleProperties::set_face_to(state, &self.role_id, &Vec3::sub(&new_position, &position));

        completed
    }
}

impl SceCommandRolePathTo {
    pub fn new(res_man: &Rc<ResourceManager>, role_id: i32, from: Vec3, to: Vec3) -> Self {
        Self {
            res_man: res_man.clone(),
            role_id: format!("{}", role_id),
            from,
            to,
        }
    }
}
