use crate::math::Transform;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub trait Entity {
    fn load(&mut self);
    fn update(&mut self, delta_sec: f32);
    fn transform(&self) -> &Transform;
    fn transform_mut(&mut self) -> &mut Transform;
    fn add_component(&mut self, component: Box<dyn Any>);
    fn get_component(&self, type_id: TypeId) -> Option<&dyn Any>;
    fn get_component_mut(&mut self, type_id: TypeId) -> Option<&mut dyn Any>;
    fn component_do2(
        &mut self,
        type_id1: TypeId,
        type_id2: TypeId,
        action: &dyn Fn(&mut dyn Any, &mut dyn Any),
    ) -> Option<()>;
}

pub trait EntityCallbacks {
    define_callback_fn!(on_loading, CoreEntity, EntityCallbacks);
    define_callback_fn!(on_updating, CoreEntity, EntityCallbacks, _delta_sec: f32);
}

pub struct CoreEntity<TCallbacks: EntityCallbacks> {
    transform: Transform,
    components: HashMap<TypeId, Vec<Box<dyn Any>>>,
    callbacks: Rc<RefCell<TCallbacks>>,
}

impl<TCallbacks: EntityCallbacks> CoreEntity<TCallbacks> {
    pub fn new(callbacks: TCallbacks) -> Self {
        Self {
            transform: Transform::new(),
            components: HashMap::new(),
            callbacks: Rc::new(RefCell::new(callbacks)),
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
    let component = entity.get_component(type_id);
    let a = component.unwrap();
    let x = a.downcast_ref::<T>().unwrap();
    Some(x)
}

impl<TCallbacks: EntityCallbacks> Entity for CoreEntity<TCallbacks> {
    fn load(&mut self) {
        callback!(self, on_loading);
    }

    fn update(&mut self, delta_sec: f32) {
        callback!(self, on_updating, delta_sec);
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

    fn component_do2(
        &mut self,
        type_id1: TypeId,
        type_id2: TypeId,
        action: &dyn Fn(&mut dyn Any, &mut dyn Any),
    ) -> Option<()> {
        let component1 = unsafe { &mut *(Entity::get_component_mut(self, type_id1)? as *mut _) };

        let component2 = unsafe { &mut *(Entity::get_component_mut(self, type_id2)? as *mut _) };

        action(component1, component2);
        Some(())
    }
}
