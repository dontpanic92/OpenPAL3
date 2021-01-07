use std::{cell::RefCell, rc::Rc};

use super::{shared_state::SharedState, SceneManagerExtensions};
use log::debug;
use radiance::{
    input::{InputEngine, Key},
    math::Vec3,
    scene::{Director, Entity, SceneManager},
};

use super::SceDirector;

pub struct ExplorationDirector {
    sce_director: Rc<RefCell<SceDirector>>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
    shared_state: Rc<RefCell<SharedState>>,
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
        }
    }
}

impl Director for ExplorationDirector {
    fn activate(&mut self, scene_manager: &mut dyn SceneManager) {
        debug!("ExplorationDirector activated");
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut imgui::Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        self.shared_state.borrow_mut().update(delta_sec);

        let input = self.input_engine.borrow_mut();
        let mut direction = Vec3::new(0., 0., 0.);

        if input.get_key_state(Key::Up).is_down() {
            debug!("Key up is down");
            direction = Vec3::add(&direction, &Vec3::new(0., 0., -1.));
        }

        if input.get_key_state(Key::Down).is_down() {
            debug!("Key down is down");
            direction = Vec3::add(&direction, &Vec3::new(0., 0., 1.));
        }

        if input.get_key_state(Key::Left).is_down() {
            debug!("Key left is down");
            direction = Vec3::add(&direction, &Vec3::new(-1., 0., 0.));
        }

        if input.get_key_state(Key::Right).is_down() {
            debug!("Key right is down");
            direction = Vec3::add(&direction, &Vec3::new(1., 0., 0.));
        }

        direction.normalize();

        let scene = scene_manager.scene_mut_or_fail();
        let position = scene.get_role_entity("101").transform().position();

        let speed = 175.;
        let target_position = Vec3::add(&position, &Vec3::dot(speed * delta_sec, &direction));
        let distance_to_border = scene.get_distance_to_border_by_scene_coord(&target_position);

        if let Some(proc_id) = scene.try_trigger_sce_proc(&target_position) {
            debug!("New proc triggerd: {}", proc_id);
            self.sce_director.borrow_mut().call_proc(proc_id);
            return Some(self.sce_director.clone());
        }

        let entity = scene.get_role_entity_mut("101");
        if direction.norm() > 0.5 && distance_to_border > std::f32::EPSILON {
            entity.run();
            entity
                .transform_mut()
                .look_at(&target_position)
                .set_position(&target_position);
        } else {
            entity.idle();
        }

        None
    }
}
