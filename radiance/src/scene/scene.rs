use super::{
    entity::{CoreEntity, Entity, EntityExtension},
    Camera,
};
use std::cell::RefCell;
use std::{f32::consts::PI, rc::Rc};

pub trait Scene: downcast_rs::Downcast {
    fn load(&mut self);
    fn update(&mut self, delta_sec: f32);
    fn draw_ui(&mut self, ui: &mut imgui::Ui);
    fn unload(&mut self);
    fn entities(&self) -> &Vec<Box<dyn Entity>>;
    fn entities_mut(&mut self) -> &mut Vec<Box<dyn Entity>>;
    fn camera(&self) -> &Camera;
    fn camera_mut(&mut self) -> &mut Camera;
}

downcast_rs::impl_downcast!(Scene);

pub trait SceneExtension<TImpl: SceneExtension<TImpl>> {
    define_ext_fn!(on_loading, CoreScene, TImpl);
    define_ext_fn!(on_loaded, CoreScene, TImpl);
    define_ext_fn!(on_updating, CoreScene, TImpl, _delta_sec: f32);
    define_ext_fn!(on_updated, CoreScene, TImpl, _delta_sec: f32);
    define_ext_fn!(on_ui_drawing, CoreScene, TImpl);
}

pub struct CoreScene<TExtension: SceneExtension<TExtension>> {
    entities: Vec<Box<dyn Entity>>,
    extension: Rc<RefCell<TExtension>>,
    camera: Camera,
}

impl<TExtension: SceneExtension<TExtension>> CoreScene<TExtension> {
    pub fn new(ext_calls: TExtension) -> Self {
        Self {
            entities: vec![],
            extension: Rc::new(RefCell::new(ext_calls)),
            camera: Camera::new(),
        }
    }

    pub fn add_entity<T: EntityExtension + 'static>(&mut self, entity: CoreEntity<T>) {
        self.entities.push(Box::new(entity));
    }

    pub fn extension(&self) -> &Rc<RefCell<TExtension>> {
        &self.extension
    }
}

mod private {
    pub struct DefaultExtension {}
    impl super::SceneExtension<DefaultExtension> for DefaultExtension {}
}

pub type DefaultScene = CoreScene<private::DefaultExtension>;

impl<TExtension: 'static + SceneExtension<TExtension>> Scene for CoreScene<TExtension> {
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

    fn draw_ui(&mut self, ui: &mut imgui::Ui) {
        ext_call!(self, on_ui_drawing);
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
