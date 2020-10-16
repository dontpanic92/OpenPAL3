use crate::math::Transform;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

/*pub trait Component: downcast_rs::Downcast {
    fn into_component(self: Box<Self>) -> Box<dyn Component>;
}

downcast_rs::impl_downcast!(Component);*/

pub trait Entity: downcast_rs::Downcast {
    fn name(&self) -> &str;
    fn set_name(&mut self, name: &str);
    fn load(&mut self);
    fn update(&mut self, delta_sec: f32);
    fn transform(&self) -> &Transform;
    fn transform_mut(&mut self) -> &mut Transform;
    fn add_component(&mut self, type_id: TypeId, component: Box<dyn Any>);
    fn get_component(&self, type_id: TypeId) -> Option<&Box<dyn Any>>;
    fn get_component_mut(&mut self, type_id: TypeId) -> Option<&mut Box<dyn Any>>;
    fn remove_component(&mut self, type_id: TypeId);
}

downcast_rs::impl_downcast!(Entity);

pub trait EntityExtension {
    fn on_loading(self: &mut CoreEntity<Self>)
    where
        Self: Sized + 'static,
    {
    }

    fn on_updating(self: &mut CoreEntity<Self>, _delta_sec: f32)
    where
        Self: Sized + 'static,
    {
    }
}

pub struct CoreEntity<TExtension: EntityExtension> {
    name: String,
    transform: Transform,
    components: HashMap<TypeId, Vec<Box<dyn Any>>>,
    extension: TExtension,
}

impl<TExtension: EntityExtension + 'static> CoreEntity<TExtension> {
    pub fn new(extension: TExtension, name: &str) -> Self {
        Self {
            name: name.to_owned(),
            transform: Transform::new(),
            components: HashMap::new(),
            extension,
        }
    }

    pub fn add_component<T: 'static>(&mut self, component: Box<T>) {
        <Self as Entity>::add_component(self, TypeId::of::<T>(), component);
    }

    /*pub fn add_component_as<TKey: 'static + ?Sized>(&mut self, component: Box<dyn Component>) {
        <Self as Entity>::add_component(self, TypeId::of::<TKey>(), component);
    }*/

    pub fn get_component<TKey: 'static>(&self) -> Option<&TKey> {
        let type_id = TypeId::of::<TKey>();
        let component = Entity::get_component(self, type_id);
        component.and_then(|c| c.downcast_ref())
    }

    pub fn get_component_mut<TKey: 'static>(&mut self) -> Option<&mut TKey> {
        let type_id = TypeId::of::<TKey>();
        let component = Entity::get_component_mut(self, type_id);
        component.and_then(|c| c.downcast_mut())
    }

    pub fn remove_component<T: 'static + ?Sized>(&mut self) {
        let type_id = TypeId::of::<T>();
        Entity::remove_component(self, type_id);
    }
}

impl<TExtension: EntityExtension + 'static> Deref for CoreEntity<TExtension> {
    type Target = TExtension;

    #[inline(always)]
    fn deref(&self) -> &TExtension {
        &self.extension
    }
}

impl<TExtension: EntityExtension + 'static> DerefMut for CoreEntity<TExtension> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut TExtension {
        &mut self.extension
    }
}

#[inline]
pub fn entity_add_component<T: 'static>(entity: &mut dyn Entity, component: T) {
    entity.add_component(TypeId::of::<T>(), Box::new(component));
}

#[inline]
pub fn entity_add_component_as<TKey: 'static, T: 'static>(entity: &mut dyn Entity, component: T) {
    entity.add_component(TypeId::of::<TKey>(), Box::new(component));
}

#[inline]
pub fn entity_get_component<T: 'static>(entity: &dyn Entity) -> Option<&T> {
    let type_id = TypeId::of::<T>();
    entity
        .get_component(type_id)
        .and_then(|component| component.downcast_ref())
}

#[inline]
pub fn entity_remove_component<T: 'static>(entity: &mut dyn Entity) {
    let type_id = TypeId::of::<T>();
    entity.remove_component(type_id)
}

impl<TExtension: EntityExtension + 'static> Entity for CoreEntity<TExtension> {
    fn name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
    }

    fn load(&mut self) {
        self.on_loading();
    }

    fn update(&mut self, delta_sec: f32) {
        self.on_updating(delta_sec);
    }

    fn transform(&self) -> &Transform {
        &self.transform
    }

    fn transform_mut(&mut self) -> &mut Transform {
        &mut self.transform
    }

    fn add_component(&mut self, type_id: TypeId, component: Box<dyn Any>) {
        if !self.components.contains_key(&type_id) {
            self.components.insert(type_id, vec![]);
        }

        let v = self.components.get_mut(&type_id).unwrap();
        v.push(component);
    }

    fn get_component(&self, type_id: TypeId) -> Option<&Box<dyn Any>> {
        if !self.components.contains_key(&type_id) {
            return None;
        }

        self.components
            .get(&type_id)
            .filter(|v| !v.is_empty())
            .and_then(|v| Some(&v[0]))
    }

    fn get_component_mut(&mut self, type_id: TypeId) -> Option<&mut Box<dyn Any>> {
        if !self.components.contains_key(&type_id) {
            return None;
        }

        self.components
            .get_mut(&type_id)
            .filter(|v| !v.is_empty())
            .and_then(|v| Some(&mut v[0]))
    }

    fn remove_component(&mut self, type_id: TypeId) {
        if !self.components.contains_key(&type_id) {
            return;
        }

        self.components.remove(&type_id);
    }
}
