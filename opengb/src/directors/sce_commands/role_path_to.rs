use crate::directors::sce_vm::{SceCommand, SceState};

use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::Entity;
use radiance::{math::Vec3, scene::SceneManager};

#[derive(Clone)]
pub struct SceCommandRolePathTo {
    role_id: i32,
    nav_x: f32,
    nav_z: f32,
    run: i32,
}

impl SceCommand for SceCommandRolePathTo {
    fn initialize(&mut self, scene_manager: &mut dyn SceneManager, state: &mut SceState) {
        let role = scene_manager.resolve_role_mut_do(state, self.role_id, |role| {
            if self.run == 1 {
                role.run();
            } else {
                role.walk();
            }
        });
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        const WALK_SPEED: f32 = 80.;
        const RUN_SPEED: f32 = 175.;

        let speed = if self.run == 1 { RUN_SPEED } else { WALK_SPEED };
        let role = scene_manager
            .get_resolved_role(state, self.role_id)
            .unwrap();
        let to = {
            let scene = scene_manager.core_scene_or_fail();
            scene.nav_coord_to_scene_coord(role.nav_layer(), self.nav_x, self.nav_z)
        };

        let role = scene_manager
            .get_resolved_role_mut(state, self.role_id)
            .unwrap();
        let position = role.transform().position();
        let step = speed * delta_sec;
        let remain = Vec3::sub(&to, &position);
        let completed = remain.norm() < step;
        let new_position = if completed {
            to
        } else {
            Vec3::add(&position, &Vec3::dot(step, &Vec3::normalized(&remain)))
        };

        role.transform_mut()
            .look_at(&Vec3::new(to.x, position.y, to.z))
            .set_position(&new_position);

        if completed {
            role.idle();
        }
        completed
    }
}

impl SceCommandRolePathTo {
    pub fn new(role_id: i32, nav_x: i32, nav_z: i32, run: i32) -> Self {
        Self {
            role_id,
            nav_x: nav_x as f32,
            nav_z: nav_z as f32,
            run,
        }
    }
}
