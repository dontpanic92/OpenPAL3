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

pub trait SceneExtension<TImpl: SceneExtension<TImpl>> {
    define_ext_fn!(on_loading, CoreScene, TImpl);
    define_ext_fn!(on_loaded, CoreScene, TImpl);
    define_ext_fn!(on_updating, CoreScene, TImpl, _delta_sec: f32);
    define_ext_fn!(on_updated, CoreScene, TImpl, _delta_sec: f32);
}

pub struct CoreScene<TExtension: SceneExtension<TExtension>> {
    entities: Vec<Box<dyn Entity>>,
    extension: Rc<RefCell<TExtension>>,
    camera: Camera,
}

impl<TExtension: SceneExtension<TExtension>> CoreScene<TExtension> {
    pub fn new(ext_calls: TExtension, camera_aspect: f32) -> Self {
        Self {
            entities: vec![],
            extension: Rc::new(RefCell::new(ext_calls)),
            camera: Camera::new(30. * PI / 180., camera_aspect, 0.1, 100000.),
        }
    }

    pub fn add_entity<T: EntityCallbacks + 'static>(&mut self, entity: CoreEntity<T>) {
        self.entities.push(Box::new(entity));
    }
}

mod private {
    pub struct DefaultExtension {}
    impl super::SceneExtension<DefaultExtension> for DefaultExtension {}
}

pub type DefaultScene = CoreScene<private::DefaultExtension>;

impl<TExtension: SceneExtension<TExtension>> Scene for CoreScene<TExtension> {
    fn load(&mut self) {
        ext_call!(self, on_loading);
        for entity in &mut self.entities {
            entity.load();
        }

        ext_call!(self, on_loaded);
    }

    fn update(&mut self, delta_sec: f32) {
        ext_call!(self, on_updating, delta_sec);
        for e in &mut self.entities {
            e.update(delta_sec);
        }

        ext_call!(self, on_updated, delta_sec);
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
