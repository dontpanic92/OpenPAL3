use std::{cell::RefCell, rc::Rc};

use super::{shared_state::SharedState, SceneManagerExtensions};
use log::debug;
use radiance::{
    input::{InputEngine, Key},
    math::{Mat44, Vec3},
    scene::{Director, Entity, SceneManager},
};

use super::SceDirector;

pub struct ExplorationDirector {
    sce_director: Rc<RefCell<SceDirector>>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
    shared_state: Rc<RefCell<SharedState>>,
    camera_rotation: f32,
}

impl ExplorationDirector {
    pub fn new(
        sce_director: Rc<RefCell<SceDirector>>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
        shared_state: Rc<RefCell<SharedState>>,
    ) -> Self {
        Self {
            sce_director,
            input_engine,
            shared_state,
            camera_rotation: 0.,
        }
    }

    pub fn test_save(&self) {
        let input = self.input_engine.borrow_mut();
        let save_slot = if input.get_key_state(Key::Num1).pressed() {
            1
        } else if input.get_key_state(Key::Num2).pressed() {
            2
        } else if input.get_key_state(Key::Num3).pressed() {
            3
        } else if input.get_key_state(Key::Num4).pressed() {
            4
        } else {
            -1
        };

        let mut shared_state = self.shared_state.borrow_mut();
        let state = shared_state.persistent_state_mut();
        state.save(save_slot);
    }
}

impl Director for ExplorationDirector {
    fn activate(&mut self, scene_manager: &mut dyn SceneManager) {
        debug!("ExplorationDirector activated");
        scene_manager
            .core_scene_mut_or_fail()
            .get_role_entity_mut(-1)
            .set_active(true);
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut imgui::Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        self.shared_state.borrow_mut().update(delta_sec);
        self.test_save();

        let input = self.input_engine.borrow_mut();
        let mut direction = Vec3::new(0., 0., 0.);

        if input.get_key_state(Key::Up).is_down() {
            direction = Vec3::add(&direction, &Vec3::new(0., 0., -1.));
        }

        if input.get_key_state(Key::Down).is_down() {
            direction = Vec3::add(&direction, &Vec3::new(0., 0., 1.));
        }

        if input.get_key_state(Key::Left).is_down() {
            direction = Vec3::add(&direction, &Vec3::new(-1., 0., 0.));
        }

        if input.get_key_state(Key::Right).is_down() {
            direction = Vec3::add(&direction, &Vec3::new(1., 0., 0.));
        }

        let camera_mat = &scene_manager
            .scene_mut()
            .unwrap()
            .camera_mut()
            .transform()
            .matrix();
        let mut direction_mat = Mat44::new_zero();
        direction_mat[0][3] = direction.x;
        direction_mat[1][3] = direction.y;
        direction_mat[2][3] = direction.z;
        let direction_mat = Mat44::multiplied(&camera_mat, &direction_mat);
        let mut direction = Vec3::new(direction_mat[0][3], 0., direction_mat[2][3]);
        direction.normalize();

        const CAMERA_ROTATE_SPEED: f32 = 1.5;
        if input.get_key_state(Key::A).is_down() {
            self.camera_rotation -= CAMERA_ROTATE_SPEED * delta_sec;
            if self.camera_rotation < 0. {
                self.camera_rotation += std::f32::consts::PI * 2.;
            }
        }

        if input.get_key_state(Key::D).is_down() {
            self.camera_rotation += CAMERA_ROTATE_SPEED * delta_sec;

            if self.camera_rotation > std::f32::consts::PI * 2. {
                self.camera_rotation -= std::f32::consts::PI * 2.;
            }
        }

        let scene = scene_manager.core_scene_mut_or_fail();
        let position = scene.get_role_entity(-1).transform().position();
        scene_manager
            .scene_mut()
            .unwrap()
            .camera_mut()
            .transform_mut()
            .set_position(&Vec3::new(400., 400., 400.))
            .rotate_axis_angle(&Vec3::UP, self.camera_rotation)
            .translate(&position)
            .look_at(&position);

        let scene = scene_manager.core_scene_mut_or_fail();
        let speed = 175.;
        let mut target_position = Vec3::add(&position, &Vec3::dot(speed * delta_sec, &direction));
        let target_nav_coord = scene.scene_coord_to_nav_coord(&target_position);
        let height = scene.get_height(target_nav_coord);
        target_position.y = height;
        let distance_to_border = scene.get_distance_to_border_by_scene_coord(&target_position);

        if let Some(proc_id) = scene.test_nav_trigger(&target_position) {
            debug!("New proc triggerd by nav: {}", proc_id);
            self.sce_director.borrow_mut().call_proc(proc_id);
            return Some(self.sce_director.clone());
        }

        if input.get_key_state(Key::F).pressed() {
            if let Some(proc_id) = scene
                .test_aabb_trigger(&position)
                .or_else(|| scene.test_item_trigger(&position))
                .or_else(|| scene.test_role_trigger(&position))
            {
                debug!("New proc triggerd: {}", proc_id);
                self.sce_director.borrow_mut().call_proc(proc_id);
                return Some(self.sce_director.clone());
            }
        }

        let entity = scene.get_role_entity_mut(-1);
        if direction.norm() > 0.5 && distance_to_border > std::f32::EPSILON {
            entity.run();
            entity
                .transform_mut()
                .look_at(&Vec3::new(target_position.x, position.y, target_position.z))
                .set_position(&target_position);

            self.shared_state
                .borrow_mut()
                .persistent_state_mut()
                .set_position(target_position);
        } else {
            entity.idle();
        }

        None
    }
}
