use crate::math::Transform;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

pub trait Entity: downcast_rs::Downcast {
    fn name(&self) -> &str;
    fn set_name(&mut self, name: &str);
    fn load(&mut self);
    fn update(&mut self, delta_sec: f32);
    fn transform(&self) -> &Transform;
    fn transform_mut(&mut self) -> &mut Transform;
    fn add_component(&mut self, component: Box<dyn Any>);
    fn get_component(&self, type_id: TypeId) -> Option<&dyn Any>;
    fn get_component_mut(&mut self, type_id: TypeId) -> Option<&mut dyn Any>;
    fn remove_component(&mut self, type_id: TypeId);
    unsafe fn component_do2(
        &mut self,
        type_id1: TypeId,
        type_id2: TypeId,
        action: &dyn Fn(Option<&mut dyn Any>, Option<&mut dyn Any>, &mut dyn Entity),
    ) -> Option<()>;
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

    pub fn add_component<T: 'static>(&mut self, component: T) {
        <Self as Entity>::add_component(self, Box::new(component));
    }

    pub fn get_component<T: 'static>(&self) -> Option<&T> {
        let type_id = TypeId::of::<T>();
        let component = Entity::get_component(self, type_id);
        component.and_then(|c| c.downcast_ref())
    }

    pub fn get_component_mut<T: 'static>(&mut self) -> Option<&mut T> {
        let type_id = TypeId::of::<T>();
        let component = Entity::get_component_mut(self, type_id);
        component.and_then(|c| c.downcast_mut())
    }

    pub fn remove_component<T: 'static>(&mut self) {
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
    entity.add_component(Box::new(component));
}

#[inline]
pub fn entity_get_component<T: 'static>(entity: &dyn Entity) -> Option<&T> {
    let type_id = TypeId::of::<T>();
    entity
        .get_component(type_id)
        .and_then(|component| component.downcast_ref::<T>())
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

    fn add_component(&mut self, component: Box<dyn Any>) {
        let type_id = component.as_ref().type_id();
        if !self.components.contains_key(&type_id) {
            self.components.insert(type_id, vec![]);
        }

        let v = self.components.get_mut(&type_id).unwrap();
        v.push(component);
    }

    fn get_component(&self, type_id: TypeId) -> Option<&dyn Any> {
        if !self.components.contains_key(&type_id) {
            return None;
        }

        self.components
            .get(&type_id)
            .filter(|v| !v.is_empty())
            .and_then(|v| Some(v[0].as_ref()))
    }

    fn get_component_mut(&mut self, type_id: TypeId) -> Option<&mut dyn Any> {
        if !self.components.contains_key(&type_id) {
            return None;
        }

        self.components
            .get_mut(&type_id)
            .filter(|v| !v.is_empty())
            .and_then(|v| Some(v[0].as_mut()))
    }

    fn remove_component(&mut self, type_id: TypeId) {
        if !self.components.contains_key(&type_id) {
            return;
        }

        self.components.remove(&type_id);
    }

    unsafe fn component_do2(
        &mut self,
        type_id1: TypeId,
        type_id2: TypeId,
        action: &dyn Fn(Option<&mut dyn Any>, Option<&mut dyn Any>, &mut dyn Entity),
    ) -> Option<()> {
        let component1 =
            Entity::get_component_mut(self, type_id1).and_then(|c| Some(&mut *(c as *mut _)));
        let component2 =
            Entity::get_component_mut(self, type_id2).and_then(|c| Some(&mut *(c as *mut _)));

        action(component1, component2, self);
        Some(())
    }
}
