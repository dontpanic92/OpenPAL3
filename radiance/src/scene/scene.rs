use super::Entity;
use crate::math::{Vec2, Vec3};
use crate::rendering::{RenderObject, Vertex};
use std::cell::RefCell;
use std::rc::Rc;

pub trait Scene {
    fn load(&mut self);
    fn update(&mut self);
    fn unload(&mut self);
    fn entities(&self) -> &Vec<Entity>;
    fn entities_mut(&mut self) -> &mut Vec<Entity>;
}

pub trait SceneCallbacks {
    define_callback_fn!(on_loading, CoreScene, SceneCallbacks);
    define_callback_fn!(on_loaded, CoreScene, SceneCallbacks);
}

mod private {
    pub struct DefaultCallbacks {}
    impl super::SceneCallbacks for DefaultCallbacks {}
}

pub type DefaultScene = CoreScene<private::DefaultCallbacks>;

pub struct CoreScene<TCallbacks: SceneCallbacks> {
    entities: Vec<Entity>,
    callbacks: Rc<RefCell<TCallbacks>>,
}

impl<TCallbacks: SceneCallbacks> CoreScene<TCallbacks> {
    pub fn new(callbacks: RefCell<TCallbacks>) -> Self {
        Self {
            entities: vec![],
            callbacks: Rc::new(callbacks),
        }
    }

    pub fn add_entity(&mut self, entity: Entity) {
        self.entities.push(entity);
    }
}

impl<TCallbacks: SceneCallbacks> Scene for CoreScene<TCallbacks> {
    fn load(&mut self) {
        callback!(self, on_loading);
        callback!(self, on_loaded);
    }

    fn update(&mut self) {}

    fn unload(&mut self) {}

    fn entities(&self) -> &Vec<Entity> {
        &self.entities
    }

    fn entities_mut(&mut self) -> &mut Vec<Entity> {
        &mut self.entities
    }
}
