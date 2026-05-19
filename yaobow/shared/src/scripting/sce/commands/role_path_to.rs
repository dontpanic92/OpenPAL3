use crate::openpal3::directors::SceneManagerExtensions;
use crate::openpal3::scene::RoleController;
use crate::scripting::sce::{SceCommand, SceState};

use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::IEntityExt;
use radiance::comdef::ISceneManager;
use radiance::math::Vec3;

#[derive(Debug, Clone)]
pub struct SceCommandRolePathTo {
    role_id: i32,
    nav_x: f32,
    nav_z: f32,
    run: i32,
}

impl SceCommand for SceCommandRolePathTo {
    fn initialize(&mut self, scene_manager: ComRc<ISceneManager>, state: &mut SceState) {
        let _role = scene_manager.resolve_role_mut_do(state, self.role_id, |_e, r| {
            if self.run == 1 {
                r.run();
            } else {
                r.walk();
            }
        });
    }

    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        const WALK_SPEED: f32 = 80.;
        const RUN_SPEED: f32 = 175.;

        let speed = if self.run == 1 { RUN_SPEED } else { WALK_SPEED };
        let role = scene_manager.get_resolved_role(state, self.role_id);

        if role.is_none() {
            return true;
        }
        let role_controller = RoleController::get_role_controller(role.unwrap()).unwrap();

        let to = {
            let nav_layer = role_controller.with_inner::<RoleController, _, _>(|r| r.nav_layer());
            scene_manager
                .scn_scene()
                .unwrap()
                .with_inner::<crate::openpal3::scene::ScnScene, _, _>(|s| {
                    s.nav_coord_to_scene_coord(nav_layer, self.nav_x, self.nav_z)
                })
        };

        let role = scene_manager
            .get_resolved_role(state, self.role_id)
            .unwrap();

        let position = role.transform().borrow().position();

        // TODO: WHY?
        if position.x.is_nan() {
            return true;
        }
        let step = speed * delta_sec;
        let remain = Vec3::sub(&to, &position);
        let completed = remain.norm() < step;
        let new_position = if completed {
            to
        } else {
            Vec3::add(
                &position,
                &Vec3::scalar_mul(step, &Vec3::normalized(&remain)),
            )
        };

        let mut look_at = Vec3::new(to.x, position.y, to.z);
        if self.run == 2 {
            look_at = Vec3::add(&position, &Vec3::sub(&position, &to));
        }

        role.transform()
            .borrow_mut()
            .look_at(&look_at)
            .set_position(&new_position);

        if completed {
            role_controller.with_inner::<RoleController, _, _>(|r| r.idle());
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
