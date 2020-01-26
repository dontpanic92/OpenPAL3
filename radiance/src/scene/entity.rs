use crate::math::Transform;
use core::borrow::{Borrow, BorrowMut};
use std::any::{Any, TypeId};
use std::collections::HashMap;

pub struct Entity {
    transform: Transform,
    components: HashMap<TypeId, Vec<Box<dyn Any>>>,
}

impl Entity {
    pub fn new() -> Self {
        Self {
            transform: Transform::new(),
            components: HashMap::new(),
        }
    }

    pub fn transform(&self) -> &Transform {
        &self.transform
    }

    pub fn transform_mut(&mut self) -> &mut Transform {
        &mut self.transform
    }

    pub fn add_component<T>(&mut self, component: T)
    where
        T: 'static,
    {
        let type_id = TypeId::of::<T>();
        if !self.components.contains_key(&type_id) {
            self.components.insert(type_id, vec![]);
        }

        let v = self.components.get_mut(&type_id).unwrap();
        v.push(Box::new(component));
    }

    pub fn get_component<T>(&self) -> Option<&T>
    where
        T: 'static,
    {
        let type_id = TypeId::of::<T>();
        if !self.components.contains_key(&type_id) {
            return None;
        }

        let v = self.components.get(&type_id).unwrap();
        if v.is_empty() {
            return None;
        }

        let component: &dyn Any = v[0].borrow();
        component.downcast_ref()
    }

    pub fn get_component_mut<T>(&mut self) -> Option<&mut T>
    where
        T: 'static,
    {
        let type_id = TypeId::of::<T>();
        if !self.components.contains_key(&type_id) {
            return None;
        }

        let v = self.components.get_mut(&type_id).unwrap();
        if v.is_empty() {
            return None;
        }

        let component: &mut dyn Any = v[0].borrow_mut();
        component.downcast_mut()
    }
}
