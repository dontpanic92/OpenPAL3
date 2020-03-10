use super::{
    entity::{CoreEntity, Entity, EntityCallbacks},
    Camera,
};
use std::cell::RefCell;
use std::{f32::consts::PI, rc::Rc};

pub trait Scene {
    fn load(&mut self);
    fn update(&mut self, delta_sec: f32);
    fn unload(&mut self);
    fn entities(&self) -> &Vec<Box<dyn Entity>>;
    fn entities_mut(&mut self) -> &mut Vec<Box<dyn Entity>>;
    fn camera(&self) -> &Camera;
    fn camera_mut(&mut self) -> &mut Camera;
}

pub trait SceneCallbacks {
    define_callback_fn!(on_loading, CoreScene, SceneCallbacks);
    define_callback_fn!(on_loaded, CoreScene, SceneCallbacks);
    define_callback_fn!(on_updating, CoreScene, SceneCallbacks, _delta_sec: f32);
    define_callback_fn!(on_updated, CoreScene, SceneCallbacks, _delta_sec: f32);
}

pub struct CoreScene<TCallbacks: SceneCallbacks> {
    entities: Vec<Box<dyn Entity>>,
    callbacks: Rc<RefCell<TCallbacks>>,
    camera: Camera,
}

impl<TCallbacks: SceneCallbacks> CoreScene<TCallbacks> {
    pub fn new(callbacks: TCallbacks, camera_aspect: f32) -> Self {
        Self {
            entities: vec![],
            callbacks: Rc::new(RefCell::new(callbacks)),
            camera: Camera::new(
                90. * PI / 180.,
                camera_aspect,
                0.1,
                100000.,
            ),
        }
    }

    pub fn add_entity<T: EntityCallbacks + 'static>(&mut self, entity: CoreEntity<T>) {
        self.entities.push(Box::new(entity));
    }
}

mod private {
    pub struct DefaultCallbacks {}
    impl super::SceneCallbacks for DefaultCallbacks {}
}

pub type DefaultScene = CoreScene<private::DefaultCallbacks>;

impl<TCallbacks: SceneCallbacks> Scene for CoreScene<TCallbacks> {
    fn load(&mut self) {
        callback!(self, on_loading);
        for entity in &mut self.entities {
            entity.load();
        }

        callback!(self, on_loaded);
    }

    fn update(&mut self, delta_sec: f32) {
        callback!(self, on_updating, delta_sec);
        for e in &mut self.entities {
            e.update(delta_sec);
        }
        
        callback!(self, on_updated, delta_sec);
    }

    fn unload(&mut self) {}

    fn entities(&self) -> &Vec<Box<dyn Entity>> {
        &self.entities
    }

    fn entities_mut(&mut self) -> &mut Vec<Box<dyn Entity>> {
        &mut self.entities
    }
    
    fn camera(&self) -> &Camera {
        &self.camera
    }

    fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }
}
