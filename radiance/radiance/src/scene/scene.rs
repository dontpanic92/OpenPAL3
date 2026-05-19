use crosscom::ComRc;
use dashmap::DashMap;
use uuid::Uuid;

use super::entity::IEntityExt;
use crate::{
    comdef::{IComponent, IComponentContainerImpl, IEntity, IScene, ISceneImpl},
    math::Transform,
};

use super::Camera;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct SceneComponentEntry {
    component: ComRc<IComponent>,
    loaded: bool,
}

pub struct CoreScene {
    active: bool,
    visible: bool,
    entities: RefCell<Vec<ComRc<IEntity>>>,
    camera: Rc<RefCell<Camera>>,
    components: DashMap<Uuid, SceneComponentEntry>,
    loaded: Cell<bool>,
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
            loaded: Cell::new(false),
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

    fn collect_entities(entity: ComRc<IEntity>, entities: &mut Vec<ComRc<IEntity>>) {
        entities.push(entity.clone());
        for e in entity.children() {
            Self::collect_entities(e, entities);
        }
    }

    fn collect_visible_entities(entity: ComRc<IEntity>, entities: &mut Vec<ComRc<IEntity>>) {
        entities.push(entity.clone());
        for e in entity.children() {
            if e.visible() {
                Self::collect_visible_entities(e, entities);
            }
        }
    }

    // ---- inherent counterparts of formerly-IDL accessors ----

    pub fn remove_entities_by_name(&self, name: &str) -> Vec<ComRc<IEntity>> {
        let mut entities = vec![];
        let mut i = 0;
        while i < self.entities.borrow().len() {
            let e = self.entities.borrow()[i].clone();
            if e.name() == name {
                entities.push(e.clone());
                self.entities.borrow_mut().remove(i);
            } else {
                i += 1;
            }
        }

        // Unload removed entities to fire on_unloading on their
        // components symmetrically with the add path.
        if self.loaded.get() {
            for e in &entities {
                e.unload();
            }
        }

        entities
    }

    pub fn root_entities(&self) -> Vec<ComRc<IEntity>> {
        self.entities.borrow().clone()
    }

    pub fn entities(&self) -> Vec<ComRc<IEntity>> {
        let mut entities = vec![];
        for e in self.entities.borrow().clone() {
            Self::collect_entities(e, &mut entities);
        }
        entities
    }

    pub fn visible_entities(&self) -> Vec<ComRc<IEntity>> {
        let mut entities = vec![];
        for e in self.entities.borrow().clone() {
            if e.visible() {
                Self::collect_visible_entities(e, &mut entities);
            }
        }
        entities
    }

    pub fn camera(&self) -> Rc<RefCell<Camera>> {
        self.camera.clone()
    }
}

/// Extension trait exposing `CoreScene`'s formerly-IDL accessors on a
/// `ComRc<IScene>` handle.
pub trait ISceneExt {
    fn remove_entities_by_name(&self, name: &str) -> Vec<ComRc<IEntity>>;
    fn root_entities(&self) -> Vec<ComRc<IEntity>>;
    fn entities(&self) -> Vec<ComRc<IEntity>>;
    fn visible_entities(&self) -> Vec<ComRc<IEntity>>;
    fn camera(&self) -> Rc<RefCell<Camera>>;
}

impl ISceneExt for ComRc<IScene> {
    fn remove_entities_by_name(&self, name: &str) -> Vec<ComRc<IEntity>> {
        self.with_inner::<CoreScene, _, _>(|s| s.remove_entities_by_name(name))
    }
    fn root_entities(&self) -> Vec<ComRc<IEntity>> {
        self.with_inner::<CoreScene, _, _>(|s| s.root_entities())
    }
    fn entities(&self) -> Vec<ComRc<IEntity>> {
        self.with_inner::<CoreScene, _, _>(|s| s.entities())
    }
    fn visible_entities(&self) -> Vec<ComRc<IEntity>> {
        self.with_inner::<CoreScene, _, _>(|s| s.visible_entities())
    }
    fn camera(&self) -> Rc<RefCell<Camera>> {
        self.with_inner::<CoreScene, _, _>(|s| s.camera())
    }
}

impl ISceneImpl for CoreScene {
    fn load(&self) {
        if self.loaded.get() {
            return;
        }
        self.loaded.set(true);

        // Snapshot uuids so we can mutate the DashMap entries while
        // dispatching on_loading (an impl may re-enter add_component).
        let uuids: Vec<Uuid> = self.components.iter().map(|kv| *kv.key()).collect();
        for uuid in uuids {
            let component = {
                let mut entry = match self.components.get_mut(&uuid) {
                    Some(e) => e,
                    None => continue,
                };
                if entry.loaded {
                    continue;
                }
                entry.loaded = true;
                entry.component.clone()
            };
            component.on_loading();
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

        let components: Vec<ComRc<IComponent>> = self
            .components
            .iter()
            .map(|kv| kv.value().component.clone())
            .collect();
        for c in components {
            c.on_updating(delta_sec);
        }
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn unload(&self) {
        if !self.loaded.get() {
            return;
        }
        self.loaded.set(false);

        for e in self.entities.borrow().clone() {
            e.unload();
        }

        self.entities.borrow_mut().clear();

        // Drain components while firing on_unloading on the ones we
        // actually fired on_loading on.
        let uuids: Vec<Uuid> = self.components.iter().map(|kv| *kv.key()).collect();
        let mut to_unload: Vec<ComRc<IComponent>> = Vec::new();
        for uuid in uuids {
            if let Some((_, entry)) = self.components.remove(&uuid) {
                if entry.loaded {
                    to_unload.push(entry.component);
                }
            }
        }
        for c in to_unload {
            c.on_unloading();
        }
    }

    fn add_entity(&self, entity: ComRc<IEntity>) {
        // If the scene is already loaded, the new entity must be
        // loaded immediately to keep the exactly-once-per-(component,
        // container) contract.
        if self.loaded.get() {
            entity.load();
        }
        self.entities.borrow_mut().push(entity);
    }
}

impl IComponentContainerImpl for CoreScene {
    fn add_component(&self, uuid: uuid::Uuid, component: crosscom::ComRc<IComponent>) -> () {
        let fire_now = self.loaded.get();
        if fire_now {
            component.on_loading();
        }
        self.components.insert(
            uuid,
            SceneComponentEntry {
                component,
                loaded: fire_now,
            },
        );
    }

    fn get_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        self.components.get(&uuid).map(|e| e.component.clone())
    }

    fn remove_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        let entry = self.components.remove(&uuid).map(|(_, e)| e);
        if let Some(e) = entry {
            if e.loaded {
                e.component.on_unloading();
            }
            Some(e.component)
        } else {
            None
        }
    }
}
