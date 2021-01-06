use std::{cell::RefCell, rc::Rc};

use super::SceneManagerExtensions;
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
}

impl ExplorationDirector {
    pub fn new(
        sce_director: Rc<RefCell<SceDirector>>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
    ) -> Self {
        Self {
            sce_director,
            input_engine,
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
        debug!("Direction: {:?}", &direction);

        let entity = scene_manager.scene_mut_or_fail().get_role_entity("101");
        if direction.norm() > 0.5 {
            entity.run();

            let speed = 150.;
            let position = entity.transform().position();
            entity
                .transform_mut()
                .look_at(&Vec3::add(&position, &direction))
                .translate(&Vec3::dot(speed * delta_sec, &direction));
        } else {
            entity.idle();
        }

        None
    }
}
