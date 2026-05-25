use crosscom::ComRc;
use uuid::Uuid;

use crate::comdef::{IComponent, IComponentContainerImpl, IEntity, IEntityImpl};
use crate::math::{Mat44, Transform, Vec3};
use crate::rendering::RenderingComponent;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::rc::Rc;

use super::mutation::{ComponentBag, MutationQueue};

enum EntityPendingChange {
    AddComponent(Uuid, ComRc<IComponent>),
    RemoveComponent(Uuid),
    AttachChild(ComRc<IEntity>),
    DetachFirstByName(String),
}

pub struct CoreEntity {
    transform: Rc<RefCell<Transform>>,
    components: ComponentBag,
    loaded: Cell<bool>,
    mutations: MutationQueue<EntityPendingChange>,
    props: RefCell<CoreEntityProps>,
}

pub struct CoreEntityProps {
    name: String,
    world_transform: Transform,
    children: Vec<ComRc<IEntity>>,
    visible: bool,
    enabled: bool,

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
            components: ComponentBag::new(),
            loaded: Cell::new(false),
            mutations: MutationQueue::new(),
            props: RefCell::new(CoreEntityProps {
                name,
                world_transform: Transform::new(),
                children: vec![],
                visible,
                enabled: true,

                rendering_component: None,
            }),
        }
    }

    pub fn props(&self) -> Ref<'_, CoreEntityProps> {
        self.props.borrow()
    }

    pub fn props_mut(&self) -> RefMut<'_, CoreEntityProps> {
        self.props.borrow_mut()
    }

    pub fn detach_first(&self, name: &str) -> Option<ComRc<IEntity>> {
        self.apply_or_queue_detach_first_by_name(name)
    }

    fn detach_first_immediate(&self, name: &str) -> Option<ComRc<IEntity>> {
        let position = {
            let props = self.props();
            props.children.iter().position(|e| e.name() == name)
        };
        position.map(|p| self.props_mut().children.remove(p))
    }

    fn attach_child_immediate(&self, child: ComRc<IEntity>) {
        self.props_mut().children.push(child);
    }

    fn apply_or_queue_add_component(&self, uuid: Uuid, component: ComRc<IComponent>) {
        if self.mutations.is_iterating() {
            self.mutations
                .enqueue(EntityPendingChange::AddComponent(uuid, component));
        } else {
            self.components.insert(uuid, component, self.loaded.get());
        }
    }

    fn apply_or_queue_remove_component(&self, uuid: Uuid) -> Option<ComRc<IComponent>> {
        if self.mutations.is_iterating() {
            let (component, was_loaded) = self.components.mark_pending_removal(uuid)?;
            if was_loaded {
                component.on_unloading();
            }
            self.mutations
                .enqueue(EntityPendingChange::RemoveComponent(uuid));
            Some(component)
        } else {
            let entry = self.components.shift_remove(uuid)?;
            if entry.loaded {
                entry.component.on_unloading();
            }
            Some(entry.component)
        }
    }

    fn apply_or_queue_attach_child(&self, child: ComRc<IEntity>) {
        if self.mutations.is_iterating() {
            self.mutations
                .enqueue(EntityPendingChange::AttachChild(child));
            return;
        }

        self.attach_child_immediate(child);
    }

    fn apply_or_queue_detach_first_by_name(&self, name: &str) -> Option<ComRc<IEntity>> {
        if self.mutations.is_iterating() {
            let child = {
                let props = self.props();
                props.children.iter().find(|e| e.name() == name).cloned()
            };
            if child.is_some() {
                self.mutations
                    .enqueue(EntityPendingChange::DetachFirstByName(name.to_owned()));
            }
            return child;
        }

        self.detach_first_immediate(name)
    }

    fn drain_pending(&self) {
        self.mutations.drain(|change| match change {
            EntityPendingChange::AddComponent(uuid, component) => {
                self.components.insert(uuid, component, self.loaded.get())
            }
            EntityPendingChange::RemoveComponent(uuid) => {
                self.components.shift_remove(uuid);
            }
            EntityPendingChange::AttachChild(child) => self.attach_child_immediate(child),
            EntityPendingChange::DetachFirstByName(name) => {
                let _ = self.detach_first_immediate(&name);
            }
        });
    }

    // ---- inherent counterparts of the formerly-IDL accessors ----
    //
    // These methods previously lived on `IEntity` via `[internal(),
    // rust()]` shims. They are now pure-Rust inherent methods on
    // `CoreEntity`; callers holding a `ComRc<IEntity>` reach them via
    // the [`IEntityExt`] extension trait.

    pub fn name(&self) -> String {
        self.props().name.clone()
    }

    pub fn set_name(&self, name: &str) {
        self.props_mut().name = name.to_owned();
    }

    pub fn transform(&self) -> Rc<RefCell<Transform>> {
        self.transform.clone()
    }

    pub fn world_transform(&self) -> Transform {
        self.props().world_transform.clone()
    }

    pub fn update_world_transform(&self, parent_transform: &Transform) {
        let mut props = self.props_mut();

        props.world_transform.set_matrix(Mat44::multiplied(
            parent_transform.matrix(),
            self.transform.borrow().matrix(),
        ));

        for e in &props.children {
            e.update_world_transform(&props.world_transform);
        }
    }

    pub fn children(&self) -> Vec<ComRc<IEntity>> {
        self.props().children.clone()
    }

    pub fn get_rendering_component(&self) -> Option<Rc<RenderingComponent>> {
        self.props().rendering_component.clone()
    }

    pub fn set_rendering_component(&self, component: Option<Rc<RenderingComponent>>) {
        self.props_mut().rendering_component = component;
    }
}

/// Extension trait that re-exposes `CoreEntity`'s formerly-IDL
/// accessors on a `ComRc<IEntity>` handle. Method names match the
/// previous IDL surface so existing callers compile unchanged once
/// they import this trait (re-exported from `radiance::comdef`).
pub trait IEntityExt {
    fn name(&self) -> String;
    fn set_name(&self, name: &str);
    fn transform(&self) -> Rc<RefCell<Transform>>;
    fn world_transform(&self) -> Transform;
    fn update_world_transform(&self, parent_transform: &Transform);
    fn children(&self) -> Vec<ComRc<IEntity>>;
    fn get_rendering_component(&self) -> Option<Rc<RenderingComponent>>;
    fn set_rendering_component(&self, component: Option<Rc<RenderingComponent>>);
}

impl IEntityExt for ComRc<IEntity> {
    fn name(&self) -> String {
        self.inner::<CoreEntity>().name()
    }
    fn set_name(&self, name: &str) {
        self.inner::<CoreEntity>().set_name(name)
    }
    fn transform(&self) -> Rc<RefCell<Transform>> {
        self.inner::<CoreEntity>().transform()
    }
    fn world_transform(&self) -> Transform {
        self.inner::<CoreEntity>().world_transform()
    }
    fn update_world_transform(&self, parent_transform: &Transform) {
        self.inner::<CoreEntity>()
            .update_world_transform(parent_transform)
    }
    fn children(&self) -> Vec<ComRc<IEntity>> {
        self.inner::<CoreEntity>().children()
    }
    fn get_rendering_component(&self) -> Option<Rc<RenderingComponent>> {
        self.inner::<CoreEntity>().get_rendering_component()
    }
    fn set_rendering_component(&self, component: Option<Rc<RenderingComponent>>) {
        self.inner::<CoreEntity>()
            .set_rendering_component(component)
    }
}

impl IComponentContainerImpl for CoreEntity {
    fn add_component(&self, uuid: uuid::Uuid, component: crosscom::ComRc<IComponent>) -> () {
        // Fire on_loading immediately only if the entity itself is
        // already loaded; otherwise defer to `IEntity::load`. This
        // makes on_loading exactly-once per (component, entity)
        // regardless of attach-vs-load ordering.
        let fire_now = self.loaded.get();
        if fire_now {
            component.on_loading();
        }
        self.apply_or_queue_add_component(uuid, component);
    }

    fn get_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        self.components.get(uuid)
    }

    fn remove_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        self.apply_or_queue_remove_component(uuid)
    }
}

impl IEntityImpl for CoreEntity {
    fn load(&self) -> crosscom::Void {
        if self.loaded.get() {
            return;
        }

        self.loaded.set(true);

        for e in self.props().children.clone() {
            e.load();
        }

        self.components.load_all();
    }

    fn unload(&self) -> () {
        if !self.loaded.get() {
            return;
        }
        self.loaded.set(false);

        for e in self.props().children.clone() {
            e.unload();
        }

        let unloading = self.components.drain_loaded_for_unload();
        for c in unloading {
            c.on_unloading();
        }

        self.props_mut().children.clear();
    }

    fn update(&self, delta_sec: f32) -> crosscom::Void {
        if !self.enabled() {
            return;
        }

        let guard = self.mutations.iter_guard();
        let children_len = self.props().children.len();
        for i in 0..children_len {
            let child = { self.props().children[i].clone() };
            child.update(delta_sec);
        }
        self.components
            .dispatch_each(|component| component.on_updating(delta_sec));
        drop(guard);
        if !self.mutations.is_iterating() {
            self.drain_pending();
        }
    }

    fn visible(&self) -> bool {
        self.props().visible
    }

    fn set_visible(&self, visible: bool) -> () {
        self.props_mut().visible = visible;
    }

    fn attach(&self, child: ComRc<IEntity>) -> () {
        // Mirror add_component's exactly-once contract: if the parent
        // is loaded, the new child must also be loaded immediately.
        if self.loaded.get() {
            child.load();
        }
        self.apply_or_queue_attach_child(child);
    }

    fn enabled(&self) -> bool {
        self.props().enabled
    }

    fn set_enabled(&self, enabled: bool) -> () {
        self.props_mut().enabled = enabled;
    }

    fn position_x(&self) -> f32 {
        self.transform.borrow().position().x
    }

    fn position_y(&self) -> f32 {
        self.transform.borrow().position().y
    }

    fn position_z(&self) -> f32 {
        self.transform.borrow().position().z
    }

    fn set_position(&self, x: f32, y: f32, z: f32) -> () {
        self.transform
            .borrow_mut()
            .set_position(&Vec3::new(x, y, z));
    }

    fn look_at(&self, x: f32, y: f32, z: f32) -> () {
        self.transform.borrow_mut().look_at(&Vec3::new(x, y, z));
    }

    fn set_position_and_look_at(&self, px: f32, py: f32, pz: f32, lx: f32, ly: f32, lz: f32) -> () {
        self.transform
            .borrow_mut()
            .set_position(&Vec3::new(px, py, pz))
            .look_at(&Vec3::new(lx, ly, lz));
    }
}
