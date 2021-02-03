use crate::math::Transform;

use super::{entity::Entity, Camera};
use std::ops::{Deref, DerefMut};

pub trait Scene: downcast_rs::Downcast {
    fn load(&mut self);
    fn update(&mut self, delta_sec: f32);
    fn draw_ui(&mut self, ui: &mut imgui::Ui);
    fn unload(&mut self);
    fn add_entity(&mut self, entity: Box<dyn Entity>);
    fn entities(&self) -> Vec<&dyn Entity>;
    fn root_entities(&self) -> &Vec<Box<dyn Entity>>;
    fn root_entities_mut(&mut self) -> &mut Vec<Box<dyn Entity>>;
    fn camera(&self) -> &Camera;
    fn camera_mut(&mut self) -> &mut Camera;
}

downcast_rs::impl_downcast!(Scene);

pub trait SceneExtension {
    fn on_loading(self: &mut CoreScene<Self>)
    where
        Self: Sized + 'static,
    {
    }

    fn on_loaded(self: &mut CoreScene<Self>)
    where
        Self: Sized + 'static,
    {
    }

    fn on_updating(self: &mut CoreScene<Self>, _delta_sec: f32)
    where
        Self: Sized + 'static,
    {
    }

    fn on_updated(self: &mut CoreScene<Self>, _delta_sec: f32)
    where
        Self: Sized + 'static,
    {
    }

    fn on_ui_drawing(self: &mut CoreScene<Self>)
    where
        Self: Sized + 'static,
    {
    }
}

pub struct CoreScene<TExtension: SceneExtension> {
    entities: Vec<Box<dyn Entity>>,
    extension: TExtension,
    camera: Camera,
}

impl<TExtension: SceneExtension> CoreScene<TExtension> {
    pub fn new(ext_calls: TExtension) -> Self {
        Self {
            entities: vec![],
            extension: ext_calls,
            camera: Camera::new(),
        }
    }

    pub fn extension(&self) -> &TExtension {
        &self.extension
    }

    pub fn extension_mut(&mut self) -> &mut TExtension {
        &mut self.extension
    }

    fn collect_entities(entity: &dyn Entity) -> Vec<&dyn Entity> {
        let mut entities = vec![];
        entities.push(entity);
        for e in entity.children() {
            entities.append(&mut Self::collect_entities(e));
        }

        entities
    }
}

impl<TExtension: SceneExtension + 'static> Deref for CoreScene<TExtension> {
    type Target = TExtension;

    #[inline(always)]
    fn deref(&self) -> &TExtension {
        &self.extension
    }
}

impl<TExtension: SceneExtension + 'static> DerefMut for CoreScene<TExtension> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut TExtension {
        &mut self.extension
    }
}

mod private {
    pub struct DefaultExtension {}
    impl super::SceneExtension for DefaultExtension {}
}

pub struct DefaultScene;
impl DefaultScene {
    pub fn create() -> CoreScene<private::DefaultExtension> {
        CoreScene::<private::DefaultExtension>::new(private::DefaultExtension{})
    }
}

impl<TExtension: 'static + SceneExtension> Scene for CoreScene<TExtension> {
    fn load(&mut self) {
        self.on_loading();
        for entity in &mut self.entities {
            entity.load();
        }

        self.on_loaded();
    }

    fn update(&mut self, delta_sec: f32) {
        self.on_updating(delta_sec);
        for e in &mut self.entities {
            e.update(delta_sec);
        }

        for e in &mut self.entities {
            e.update_world_transform(&Transform::new());
        }

        self.on_updated(delta_sec);
    }

    fn draw_ui(&mut self, ui: &mut imgui::Ui) {
        self.on_ui_drawing();
    }

    fn unload(&mut self) {}

    fn add_entity(&mut self, entity: Box<dyn Entity>) {
        self.entities.push(entity);
    }

    fn root_entities(&self) -> &Vec<Box<dyn Entity>> {
        &self.entities
    }

    fn root_entities_mut(&mut self) -> &mut Vec<Box<dyn Entity>> {
        &mut self.entities
    }

    fn entities(&self) -> Vec<&dyn Entity> {
        let mut entities = vec![];
        for e in &self.entities {
            entities.append(&mut Self::collect_entities(e.as_ref()));
        }

        entities
    }

    fn camera(&self) -> &Camera {
        &self.camera
    }

    fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }
}
