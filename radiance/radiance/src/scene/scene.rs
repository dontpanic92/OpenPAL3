use crosscom::ComRc;
use uuid::Uuid;

use super::{
    Camera,
    entity::IEntityExt,
    mutation::{ComponentBag, MutationQueue},
};
use crate::{
    comdef::{IComponent, IComponentContainerImpl, IEntity, IScene, ISceneImpl},
    math::Transform,
};
use std::cell::{Cell, Ref, RefCell, RefMut};

enum ScenePendingChange {
    AddEntity(ComRc<IEntity>),
    RemoveEntitiesByName(String),
    AddComponent(Uuid, ComRc<IComponent>),
    RemoveComponent(Uuid),
}

pub struct CoreScene {
    active: bool,
    visible: bool,
    entities: RefCell<Vec<ComRc<IEntity>>>,
    camera: RefCell<Camera>,
    lighting: RefCell<crate::scene::SceneLighting>,
    components: ComponentBag,
    loaded: Cell<bool>,
    mutations: MutationQueue<ScenePendingChange>,
}

ComObject_Scene!(super::CoreScene);

impl CoreScene {
    pub fn new() -> Self {
        Self {
            active: true,
            visible: true,
            entities: RefCell::new(vec![]),
            camera: RefCell::new(Camera::new()),
            lighting: RefCell::new(crate::scene::SceneLighting::default()),
            components: ComponentBag::new(),
            loaded: Cell::new(false),
            mutations: MutationQueue::new(),
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

    fn do_remove_entities_by_name_immediate(&self, name: &str) -> Vec<ComRc<IEntity>> {
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

        if self.loaded.get() {
            for e in &entities {
                e.unload();
            }
        }

        entities
    }

    fn apply_or_queue_add_entity(&self, entity: ComRc<IEntity>) {
        if self.mutations.is_iterating() {
            self.mutations
                .enqueue(ScenePendingChange::AddEntity(entity));
        } else {
            if self.loaded.get() {
                entity.load();
            }
            self.entities.borrow_mut().push(entity);
        }
    }

    fn apply_or_queue_remove_entities_by_name(&self, name: &str) -> Vec<ComRc<IEntity>> {
        if self.mutations.is_iterating() {
            let entities: Vec<_> = self
                .entities
                .borrow()
                .iter()
                .filter(|entity| entity.name() == name)
                .cloned()
                .collect();
            if entities.is_empty() {
                return entities;
            }

            // Defer both the structural removal and entity.unload(). Unlike
            // component removal, unloading immediately would tear down the
            // entity's components before its already-snapshotted update work
            // finishes later in the current tick.
            self.mutations
                .enqueue(ScenePendingChange::RemoveEntitiesByName(name.to_string()));
            entities
        } else {
            self.do_remove_entities_by_name_immediate(name)
        }
    }

    fn apply_or_queue_add_component(&self, uuid: Uuid, component: ComRc<IComponent>) {
        let fire_now = self.loaded.get();
        if fire_now {
            component.on_loading();
        }
        if self.mutations.is_iterating() {
            self.mutations
                .enqueue(ScenePendingChange::AddComponent(uuid, component));
        } else {
            self.components.insert(uuid, component, fire_now);
        }
    }

    fn apply_or_queue_remove_component(&self, uuid: Uuid) -> Option<ComRc<IComponent>> {
        if self.mutations.is_iterating() {
            let (component, was_loaded) = self.components.mark_pending_removal(uuid)?;
            if was_loaded {
                component.on_unloading();
            }
            self.mutations
                .enqueue(ScenePendingChange::RemoveComponent(uuid));
            Some(component)
        } else {
            let entry = self.components.shift_remove(uuid)?;
            if entry.loaded {
                entry.component.on_unloading();
            }
            Some(entry.component)
        }
    }

    fn drain_pending(&self) {
        self.mutations.drain(|change| match change {
            ScenePendingChange::AddEntity(entity) => self.apply_or_queue_add_entity(entity),
            ScenePendingChange::RemoveEntitiesByName(name) => {
                self.do_remove_entities_by_name_immediate(&name);
            }
            ScenePendingChange::AddComponent(uuid, component) => {
                self.apply_or_queue_add_component(uuid, component);
            }
            ScenePendingChange::RemoveComponent(uuid) => {
                self.components.shift_remove(uuid);
            }
        });
    }

    // ---- inherent counterparts of formerly-IDL accessors ----

    pub fn remove_entities_by_name(&self, name: &str) -> Vec<ComRc<IEntity>> {
        self.apply_or_queue_remove_entities_by_name(name)
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

    pub fn camera(&self) -> Ref<'_, Camera> {
        self.camera.borrow()
    }

    pub fn camera_mut(&self) -> RefMut<'_, Camera> {
        self.camera.borrow_mut()
    }

    pub fn lighting(&self) -> Ref<'_, crate::scene::SceneLighting> {
        self.lighting.borrow()
    }

    pub fn set_lighting(&self, lighting: crate::scene::SceneLighting) {
        *self.lighting.borrow_mut() = lighting;
    }
}

/// Extension trait exposing `CoreScene`'s formerly-IDL accessors on a
/// `ComRc<IScene>` handle.
pub trait ISceneExt {
    fn remove_entities_by_name(&self, name: &str) -> Vec<ComRc<IEntity>>;
    fn root_entities(&self) -> Vec<ComRc<IEntity>>;
    fn entities(&self) -> Vec<ComRc<IEntity>>;
    fn visible_entities(&self) -> Vec<ComRc<IEntity>>;

    /// Borrow the scene's `Camera`. The returned `Ref` lifetime is
    /// tied to `&self`, so the camera handle cannot outlive the
    /// `ComRc<IScene>` borrow. The camera is engine-internal and is
    /// never handed out as an `Rc<RefCell<…>>`.
    fn camera(&self) -> Ref<'_, Camera>;

    /// Mutable counterpart to [`camera`].
    fn camera_mut(&self) -> RefMut<'_, Camera>;

    /// Get-or-create the scene's [`CollisionWorldComponent`] — the
    /// scene-level aggregator for colliders and trigger volumes. The
    /// component is hosted on the scene itself; the first call attaches
    /// it. Returns the same instance on subsequent calls.
    fn collision_world(&self) -> ComRc<crate::comdef::ICollisionWorldComponent>;

    /// Borrow the scene's lighting environment (ambient + point lights)
    /// consumed by dynamically-lit objects.
    fn lighting(&self) -> Ref<'_, crate::scene::SceneLighting>;

    /// Replace the scene's lighting environment.
    fn set_lighting(&self, lighting: crate::scene::SceneLighting);
}

impl ISceneExt for ComRc<IScene> {
    fn remove_entities_by_name(&self, name: &str) -> Vec<ComRc<IEntity>> {
        self.inner::<CoreScene>().remove_entities_by_name(name)
    }
    fn root_entities(&self) -> Vec<ComRc<IEntity>> {
        self.inner::<CoreScene>().root_entities()
    }
    fn entities(&self) -> Vec<ComRc<IEntity>> {
        self.inner::<CoreScene>().entities()
    }
    fn visible_entities(&self) -> Vec<ComRc<IEntity>> {
        self.inner::<CoreScene>().visible_entities()
    }
    fn camera(&self) -> Ref<'_, Camera> {
        self.inner::<CoreScene>().camera()
    }
    fn camera_mut(&self) -> RefMut<'_, Camera> {
        self.inner::<CoreScene>().camera_mut()
    }
    fn collision_world(&self) -> ComRc<crate::comdef::ICollisionWorldComponent> {
        use crate::comdef::ICollisionWorldComponent;
        if let Some(existing) = self.get_component(ICollisionWorldComponent::uuid()) {
            return existing
                .query_interface::<ICollisionWorldComponent>()
                .unwrap();
        }
        let world = crate::components::collision::CollisionWorldComponent::create();
        self.add_component(
            ICollisionWorldComponent::uuid(),
            world.query_interface::<IComponent>().unwrap(),
        );
        world
    }
    fn lighting(&self) -> Ref<'_, crate::scene::SceneLighting> {
        self.inner::<CoreScene>().lighting()
    }
    fn set_lighting(&self, lighting: crate::scene::SceneLighting) {
        self.inner::<CoreScene>().set_lighting(lighting)
    }
}

impl ISceneImpl for CoreScene {
    fn load(&self) {
        if self.loaded.get() {
            return;
        }
        self.loaded.set(true);
        self.components.load_all();
        for entity in self.entities.borrow().clone() {
            entity.load();
        }
    }

    fn update(&self, delta_sec: f32) {
        if !self.active {
            return;
        }
        let guard = self.mutations.iter_guard();
        let entities_len = self.entities.borrow().len();
        crate::perf::gauge("scene.root_entities", entities_len as u64);
        crate::perf::time("scene.entity_update_total_ns", || {
            for i in 0..entities_len {
                let entity = self.entities.borrow()[i].clone();
                entity.update(delta_sec);
            }
        });
        crate::perf::time("scene.entity_world_transform_total_ns", || {
            for i in 0..entities_len {
                let entity = self.entities.borrow()[i].clone();
                entity.update_world_transform(&Transform::new());
            }
        });
        self.components
            .dispatch_each(|component| component.on_updating(delta_sec));
        drop(guard);
        if !self.mutations.is_iterating() {
            self.drain_pending();
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
        let unloading = self.components.drain_loaded_for_unload();
        for c in unloading {
            c.on_unloading();
        }
    }

    fn add_entity(&self, entity: ComRc<IEntity>) {
        self.apply_or_queue_add_entity(entity);
    }
}

impl IComponentContainerImpl for CoreScene {
    fn add_component(&self, uuid: uuid::Uuid, component: crosscom::ComRc<IComponent>) {
        self.apply_or_queue_add_component(uuid, component);
    }

    fn get_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        self.components.get(uuid)
    }

    fn remove_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        self.apply_or_queue_remove_component(uuid)
    }
}
