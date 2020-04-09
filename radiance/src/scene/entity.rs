use crate::math::Transform;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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
    unsafe fn component_do2(
        &mut self,
        type_id1: TypeId,
        type_id2: TypeId,
        action: &dyn Fn(Option<&mut dyn Any>, Option<&mut dyn Any>, &mut dyn Entity),
    ) -> Option<()>;
}

downcast_rs::impl_downcast!(Entity);

pub trait EntityExtension<TImpl: EntityExtension<TImpl>> {
    define_ext_fn!(on_loading, CoreEntity, TImpl);
    define_ext_fn!(on_updating, CoreEntity, TImpl, _delta_sec: f32);
}

pub struct CoreEntity<TExtension: EntityExtension<TExtension>> {
    name: String,
    transform: Transform,
    components: HashMap<TypeId, Vec<Box<dyn Any>>>,
    extension: Rc<RefCell<TExtension>>,
}

impl<TExtension: 'static + EntityExtension<TExtension>> CoreEntity<TExtension> {
    pub fn new(extension: TExtension, name: &str) -> Self {
        Self {
            name: name.to_owned(),
            transform: Transform::new(),
            components: HashMap::new(),
            extension: Rc::new(RefCell::new(extension)),
        }
    }

    pub fn add_component<T>(&mut self, component: T)
    where
        T: 'static,
    {
        <Self as Entity>::add_component(self, Box::new(component));
    }

    pub fn get_component<T>(&self) -> Option<&T>
    where
        T: 'static,
    {
        let type_id = TypeId::of::<T>();
        let component = Entity::get_component(self, type_id);
        component.and_then(|c| c.downcast_ref())
    }

    pub fn get_component_mut<T>(&mut self) -> Option<&mut T>
    where
        T: 'static,
    {
        let type_id = TypeId::of::<T>();
        let component = Entity::get_component_mut(self, type_id);
        component.and_then(|c| c.downcast_mut())
    }

    pub fn extension(&self) -> &Rc<RefCell<TExtension>> {
        &self.extension
    }
}

#[inline]
pub fn entity_add_component<T>(entity: &mut dyn Entity, component: T)
where
    T: 'static,
{
    entity.add_component(Box::new(component));
}

#[inline]
pub fn entity_get_component<T>(entity: &dyn Entity) -> Option<&T>
where
    T: 'static,
{
    let type_id = TypeId::of::<T>();
    entity
        .get_component(type_id)
        .and_then(|component| component.downcast_ref::<T>())
}

impl<TExtension: 'static + EntityExtension<TExtension>> Entity for CoreEntity<TExtension> {
    fn name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
    }

    fn load(&mut self) {
        ext_call!(self, on_loading);
    }

    fn update(&mut self, delta_sec: f32) {
        ext_call!(self, on_updating, delta_sec);
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
