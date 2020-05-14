use super::RoleProperties;
use super::{Direction, RolePropertyNames};
use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::{Mv3AnimRepeatMode, Mv3ModelEntity, ScnScene};
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, CoreScene, Entity, Scene};

#[derive(Clone)]
pub struct SceCommandRoleActive {
    role_id: i32,
    active: i32,
}

impl SceCommand for SceCommandRoleActive {
    fn initialize(&mut self, scene: &mut CoreScene<ScnScene>, state: &mut SceState) {
        let role_id_str = format!("{}", self.role_id);
        scene
            .entities_mut()
            .retain(|e| e.name() != RolePropertyNames::name(&role_id_str));
    }

    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        if self.active == 0 || self.role_id < 0 {
            return true;
        }

        println!("role id: {}", self.role_id);

        let entity = {
            let scn_scene = scene.extension().borrow();
            let role = scn_scene
                .roles()
                .iter()
                .find(|p| p.index as i32 == self.role_id)
                .unwrap();

            let role_id_str = format!("{}", self.role_id);
            let position = Vec3::new(role.position_x, role.position_y, role.position_z);
            RoleProperties::set_position(state, &role_id_str, &position);
            RoleProperties::set_face_to(state, &role_id_str, &Direction::SOUTH);
            let mut entity = CoreEntity::new(
                Mv3ModelEntity::new_from_file(
                    state
                        .asset_mgr()
                        .mv3_path(&role.name, &role.action_name)
                        .to_str()
                        .unwrap(),
                    Mv3AnimRepeatMode::Repeat,
                ),
                &RolePropertyNames::name(&role_id_str),
            );

            entity.load();
            entity
                .transform_mut()
                .set_position(&position)
                .look_at(&Vec3::add(&position, &Direction::SOUTH));

            entity
        };

        scene.entities_mut().push(Box::new(entity));

        return true;
    }
}

impl SceCommandRoleActive {
    pub fn new(role_id: i32, active: i32) -> Self {
        println!("new SceCommandRoleActive {}", role_id);
        Self { role_id, active }
    }
}
