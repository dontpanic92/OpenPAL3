use crate::math::{Mat44, Transform};
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
    fn world_transform(&self) -> &Transform;
    fn update_world_transform(&mut self, parent_transform: &Transform);
    fn add_component(&mut self, type_id: TypeId, component: Box<dyn Any>);
    fn get_component(&self, type_id: TypeId) -> Option<&Box<dyn Any>>;
    fn get_component_mut(&mut self, type_id: TypeId) -> Option<&mut Box<dyn Any>>;
    fn remove_component(&mut self, type_id: TypeId);
    fn children(&self) -> Vec<&dyn Entity>;
    fn visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
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
    world_transform: Transform,
    components: HashMap<TypeId, Vec<Box<dyn Any>>>,
    children: Vec<Box<dyn Entity>>,
    visible: bool,
    extension: TExtension,
}

impl<TExtension: EntityExtension + 'static> CoreEntity<TExtension> {
    pub fn new(extension: TExtension, name: String) -> Self {
        Self {
            name,
            transform: Transform::new(),
            world_transform: Transform::new(),
            components: HashMap::new(),
            children: vec![],
            visible: true,
            extension,
        }
    }

    pub fn add_component<T: 'static>(&mut self, component: Box<T>) {
        <Self as Entity>::add_component(self, TypeId::of::<T>(), component);
    }

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

    pub fn remove_component<T: 'static>(&mut self) {
        let type_id = TypeId::of::<T>();
        Entity::remove_component(self, type_id);
    }

    pub fn attach(&mut self, child: Box<dyn Entity>) {
        self.children.push(child);
    }

    pub fn detach_first(&mut self, name: &str) -> Option<Box<dyn Entity>> {
        self.children
            .iter()
            .position(|e| e.name() == name)
            .and_then(|p| Some(self.children.remove(p)))
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
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

        for e in &mut self.children {
            e.load();
        }
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

    fn world_transform(&self) -> &Transform {
        &self.world_transform
    }

    fn update_world_transform(&mut self, parent_transform: &Transform) {
        self.world_transform.set_matrix(Mat44::multiplied(
            parent_transform.matrix(),
            self.transform.matrix(),
        ));

        for e in &mut self.children {
            e.update_world_transform(&self.world_transform);
        }
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

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn children(&self) -> Vec<&dyn Entity> {
        self.children.iter().map(|e| e.as_ref()).collect()
    }
}
