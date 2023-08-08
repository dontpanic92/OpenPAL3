use crosscom::ComRc;
use dashmap::DashMap;
use uuid::Uuid;

use crate::{
    comdef::{IComponent, IComponentContainerImpl, IEntity, IScene, ISceneImpl},
    math::Transform,
    ComObject_Scene,
};

use super::Camera;
use std::{cell::RefCell, rc::Rc};

pub struct CoreScene {
    active: bool,
    visible: bool,
    entities: RefCell<Vec<ComRc<IEntity>>>,
    camera: Rc<RefCell<Camera>>,
    components: DashMap<Uuid, ComRc<IComponent>>,
}

ComObject_Scene!(super::CoreScene);

impl CoreScene {
    pub fn new() -> Self {
        Self {
            active: true,
            visible: true,
            entities: RefCell::new(vec![]),
            camera: Rc::new(RefCell::new(Camera::new())),
            components: DashMap::new(),
        }
    }

    pub fn create() -> ComRc<IScene> {
        ComRc::from_object(Self::new())
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn collect_entities(entity: ComRc<IEntity>) -> Vec<ComRc<IEntity>> {
        let mut entities = vec![];
        entities.push(entity.clone());
        for e in entity.children() {
            entities.append(&mut Self::collect_entities(e));
        }

        entities
    }

    fn collect_visible_entities(entity: ComRc<IEntity>) -> Vec<ComRc<IEntity>> {
        let mut entities = vec![];
        entities.push(entity.clone());
        for e in entity.children() {
            if e.visible() {
                entities.append(&mut Self::collect_entities(e));
            }
        }

        entities
    }
}

impl ISceneImpl for CoreScene {
    fn load(&self) {
        for c in self.components.clone() {
            c.1.on_loading()
        }

        for entity in self.entities.borrow().clone() {
            entity.load();
        }
    }

    fn update(&self, delta_sec: f32) {
        if !self.active {
            return;
        }

        let entities = self.entities.borrow().clone();

        for e in &entities {
            e.update(delta_sec);
        }

        for e in &entities {
            e.update_world_transform(&Transform::new());
        }

        for c in self.components.clone() {
            c.1.on_updating(delta_sec)
        }
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn draw_ui(&self, ui: &mut imgui::Ui) {}

    fn unload(&self) {
        for e in self.entities.borrow().clone() {
            e.unload();
        }

        self.entities.borrow_mut().clear();
        self.components.clear();
    }

    fn add_entity(&self, entity: ComRc<IEntity>) {
        self.entities.borrow_mut().push(entity);
    }

    fn root_entities(&self) -> Vec<ComRc<IEntity>> {
        self.entities.borrow().clone()
    }

    fn entities(&self) -> Vec<ComRc<IEntity>> {
        let mut entities = vec![];
        for e in self.entities.borrow().clone() {
            entities.append(&mut Self::collect_entities(e));
        }

        entities
    }

    fn visible_entities(&self) -> Vec<crosscom::ComRc<crate::comdef::IEntity>> {
        let mut entities = vec![];
        for e in self.entities.borrow().clone() {
            if e.visible() {
                entities.append(&mut Self::collect_visible_entities(e));
            }
        }

        entities
    }

    fn camera(&self) -> Rc<RefCell<Camera>> {
        self.camera.clone()
    }
}

impl IComponentContainerImpl for CoreScene {
    fn add_component(&self, uuid: uuid::Uuid, component: crosscom::ComRc<IComponent>) -> () {
        self.components.insert(uuid, component);
    }

    fn get_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        self.components
            .get(&uuid)
            .and_then(|c| Some(c.value().clone()))
    }

    fn remove_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        self.components.remove(&uuid).and_then(|c| Some(c.1))
    }
}
