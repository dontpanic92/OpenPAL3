use super::entity::{CoreEntity, Entity, EntityCallbacks};
use std::cell::RefCell;
use std::rc::Rc;

pub trait Scene {
    fn load(&mut self);
    fn update(&mut self, delta_sec: f32);
    fn unload(&mut self);
    fn entities(&self) -> &Vec<Box<dyn Entity>>;
    fn entities_mut(&mut self) -> &mut Vec<Box<dyn Entity>>;
}

pub trait SceneCallbacks {
    define_callback_fn!(on_loading, CoreScene, SceneCallbacks);
    define_callback_fn!(on_loaded, CoreScene, SceneCallbacks);
}

pub struct CoreScene<TCallbacks: SceneCallbacks> {
    entities: Vec<Box<dyn Entity>>,
    callbacks: Rc<RefCell<TCallbacks>>,
}

impl<TCallbacks: SceneCallbacks> CoreScene<TCallbacks> {
    pub fn new(callbacks: TCallbacks) -> Self {
        Self {
            entities: vec![],
            callbacks: Rc::new(RefCell::new(callbacks)),
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
        for e in &mut self.entities {
            e.update(delta_sec);
        }
    }

    fn unload(&mut self) {}

    fn entities(&self) -> &Vec<Box<dyn Entity>> {
        &self.entities
    }

    fn entities_mut(&mut self) -> &mut Vec<Box<dyn Entity>> {
        &mut self.entities
    }
}
