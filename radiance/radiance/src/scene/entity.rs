use crosscom::ComRc;
use uuid::Uuid;

use crate::interfaces::{IComponent, IEntity, IEntityImpl};
use crate::math::{Mat44, Transform};
use crate::rendering::RenderingComponent;
use crate::ComObject_Entity;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;

/*pub trait Entity {
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
    fn get_component2(&self, uuid: Uuid) -> Option<&ComRc<IComponent>>;
    fn get_component_mut(&mut self, type_id: TypeId) -> Option<&mut Box<dyn Any>>;
    fn remove_component(&mut self, type_id: TypeId);
    fn children(&self) -> Vec<&dyn Entity>;
    fn visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
}*/

pub struct CoreEntity {
    transform: Rc<RefCell<Transform>>,
    props: RefCell<CoreEntityProps>,
}

pub struct CoreEntityProps {
    name: String,
    world_transform: Transform,
    components: HashMap<Uuid, ComRc<IComponent>>,
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
            props: RefCell::new(CoreEntityProps {
                name,
                world_transform: Transform::new(),
                components: HashMap::new(),
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

impl IEntityImpl for CoreEntity {
    fn name(&self) -> String {
        self.props().name.clone()
    }

    fn set_name(&self, name: &str) -> crosscom::Void {
        self.props_mut().name = name.to_owned();
    }

    fn load(&self) -> crosscom::Void {
        let components = self.props().components.clone();
        let children = self.props().children.clone();

        for c in components {
            c.1.on_loading(ComRc::from_self(self))
        }

        for e in children {
            e.load();
        }
    }

    fn update(&self, delta_sec: f32) -> crosscom::Void {
        let components = self.props().components.clone();
        for c in components {
            c.1.on_updating(ComRc::from_self(self), delta_sec);
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

        for e in props.children.clone() {
            e.update_world_transform(&props.world_transform);
        }
    }

    fn add_component(&self, uuid: uuid::Uuid, component: crosscom::ComRc<IComponent>) -> () {
        self.props_mut().components.insert(uuid, component);
    }

    fn get_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<crate::interfaces::IComponent>> {
        self.props().components.get(&uuid).cloned()
    }

    fn remove_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<crate::interfaces::IComponent>> {
        self.props_mut().components.remove(&uuid)
    }

    fn children(&self) -> Vec<crosscom::ComRc<crate::interfaces::IEntity>> {
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

    fn attach(&self, child: crosscom::ComRc<crate::interfaces::IEntity>) -> () {
        self.props_mut().children.push(child);
    }
}
