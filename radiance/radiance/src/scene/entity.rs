use crosscom::ComRc;
use uuid::Uuid;

use crate::comdef::{IComponent, IComponentContainerImpl, IEntity, IEntityImpl};
use crate::math::{Mat44, Transform};
use crate::rendering::RenderingComponent;
use crate::ComObject_Entity;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;

pub struct CoreEntity {
    transform: Rc<RefCell<Transform>>,
    components: RefCell<HashMap<Uuid, ComRc<IComponent>>>,
    props: RefCell<CoreEntityProps>,
}

pub struct CoreEntityProps {
    name: String,
    world_transform: Transform,
    children: Vec<ComRc<IEntity>>,
    visible: bool,

    rendering_component: Option<Rc<RenderingComponent>>,
}

ComObject_Entity!(super::CoreEntity);

impl CoreEntity {
    pub fn create(name: String, visible: bool) -> ComRc<IEntity> {
        ComRc::<IEntity>::from_object(Self::new(name, visible))
    }

    pub fn from_object(entity: CoreEntity) -> ComRc<IEntity> {
        ComRc::<IEntity>::from_object(entity)
    }

    pub fn new(name: String, visible: bool) -> Self {
        Self {
            transform: Rc::new(RefCell::new(Transform::new())),
            components: RefCell::new(HashMap::new()),
            props: RefCell::new(CoreEntityProps {
                name,
                world_transform: Transform::new(),
                children: vec![],
                visible,

                rendering_component: None,
            }),
        }
    }

    pub fn props(&self) -> Ref<CoreEntityProps> {
        self.props.borrow()
    }

    pub fn props_mut(&self) -> RefMut<CoreEntityProps> {
        self.props.borrow_mut()
    }

    pub fn detach_first(&mut self, name: &str) -> Option<ComRc<IEntity>> {
        self.props()
            .children
            .iter()
            .position(|e| e.name() == name)
            .and_then(|p| Some(self.props_mut().children.remove(p)))
    }
}

impl IComponentContainerImpl for CoreEntity {
    fn add_component(&self, uuid: uuid::Uuid, component: crosscom::ComRc<IComponent>) -> () {
        component.on_loading();
        self.components.borrow_mut().insert(uuid, component);
    }

    fn get_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        self.components
            .borrow()
            .get(&uuid)
            .and_then(|c| Some(c.clone()))
    }

    fn remove_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        let component = self
            .components
            .borrow_mut()
            .remove(&uuid)
            .and_then(|c| Some(c));

        if let Some(c) = &component {
            c.on_unloading();
        }

        component
    }
}

impl IEntityImpl for CoreEntity {
    fn name(&self) -> String {
        self.props().name.clone()
    }

    fn set_name(&self, name: &str) -> crosscom::Void {
        self.props_mut().name = name.to_owned();
    }

    fn load(&self) -> crosscom::Void {
        for e in self.props().children.clone() {
            e.load();
        }

        for c in self.components.borrow().clone() {
            c.1.on_loading()
        }
    }

    fn unload(&self) -> () {
        for e in self.props().children.clone() {
            e.unload();
        }

        for c in self.components.borrow().clone() {
            c.1.on_unloading()
        }

        self.props_mut().children.clear();
        self.components.borrow_mut().clear();
    }

    fn update(&self, delta_sec: f32) -> crosscom::Void {
        for e in self.props().children.clone() {
            e.update(delta_sec);
        }

        let components = self.components.borrow().clone();
        for c in components.values() {
            c.on_updating(delta_sec);
        }
    }

    fn transform(&self) -> Rc<RefCell<crate::math::Transform>> {
        self.transform.clone()
    }

    fn world_transform(&self) -> crate::math::Transform {
        self.props().world_transform.clone()
    }

    fn update_world_transform(&self, parent_transform: &crate::math::Transform) -> crosscom::Void {
        let mut props = self.props_mut();

        props.world_transform.set_matrix(Mat44::multiplied(
            parent_transform.matrix(),
            self.transform.borrow().matrix(),
        ));

        for e in &props.children {
            e.update_world_transform(&props.world_transform);
        }
    }

    fn children(&self) -> Vec<ComRc<IEntity>> {
        self.props().children.clone()
    }

    fn visible(&self) -> bool {
        self.props().visible
    }

    fn set_visible(&self, visible: bool) -> () {
        self.props_mut().visible = visible;
    }

    fn get_rendering_component(&self) -> Option<Rc<crate::rendering::RenderingComponent>> {
        self.props().rendering_component.clone()
    }

    fn set_rendering_component(
        &self,
        component: Option<Rc<crate::rendering::RenderingComponent>>,
    ) -> crosscom::Void {
        self.props_mut().rendering_component = component;
    }

    fn attach(&self, child: ComRc<IEntity>) -> () {
        self.props_mut().children.push(child);
    }
}
