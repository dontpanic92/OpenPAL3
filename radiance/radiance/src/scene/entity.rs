use crosscom::ComRc;
use uuid::Uuid;

use crate::comdef::{IComponent, IComponentContainerImpl, IEntity, IEntityImpl};
use crate::math::{Mat44, Transform};
use crate::rendering::RenderingComponent;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;

struct ComponentEntry {
    component: ComRc<IComponent>,
    loaded: bool,
}

pub struct CoreEntity {
    transform: Rc<RefCell<Transform>>,
    components: RefCell<HashMap<Uuid, ComponentEntry>>,
    loaded: Cell<bool>,
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
            components: RefCell::new(HashMap::new()),
            loaded: Cell::new(false),
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

    pub fn detach_first(&mut self, name: &str) -> Option<ComRc<IEntity>> {
        self.props()
            .children
            .iter()
            .position(|e| e.name() == name)
            .and_then(|p| Some(self.props_mut().children.remove(p)))
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
        self.components.borrow_mut().insert(
            uuid,
            ComponentEntry {
                component,
                loaded: fire_now,
            },
        );
    }

    fn get_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        self.components
            .borrow()
            .get(&uuid)
            .map(|e| e.component.clone())
    }

    fn remove_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        let entry = self.components.borrow_mut().remove(&uuid);
        if let Some(e) = &entry {
            if e.loaded {
                e.component.on_unloading();
            }
        }
        entry.map(|e| e.component)
    }
}

impl IEntityImpl for CoreEntity {
    fn load(&self) -> crosscom::Void {
        // Idempotent: a second load() call after the entity is
        // already loaded is a no-op. Newly-added components/children
        // that joined after the first load() were already loaded on
        // attach (see add_component / attach) so there's nothing to
        // fire here either.
        if self.loaded.get() {
            return;
        }

        // Flip the flag *before* dispatching so re-entrant
        // `add_component` calls from inside an `on_loading` impl see
        // the entity as loaded and fire on_loading themselves.
        self.loaded.set(true);

        for e in self.props().children.clone() {
            e.load();
        }

        // Snapshot uuids first so we don't hold the components
        // RefCell across the on_loading call (an on_loading impl may
        // re-enter add_component).
        let uuids: Vec<Uuid> = self.components.borrow().keys().copied().collect();
        for uuid in uuids {
            let component = {
                let mut comps = self.components.borrow_mut();
                let entry = comps.get_mut(&uuid).expect("entry just snapshotted");
                if entry.loaded {
                    continue;
                }
                entry.loaded = true;
                entry.component.clone()
            };
            component.on_loading();
        }
    }

    fn unload(&self) -> () {
        if !self.loaded.get() {
            return;
        }
        self.loaded.set(false);

        for e in self.props().children.clone() {
            e.unload();
        }

        let entries: Vec<ComRc<IComponent>> = self
            .components
            .borrow_mut()
            .drain()
            .filter_map(|(_uuid, e)| if e.loaded { Some(e.component) } else { None })
            .collect();
        for c in entries {
            c.on_unloading();
        }

        self.props_mut().children.clear();
    }

    fn update(&self, delta_sec: f32) -> crosscom::Void {
        if !self.enabled() {
            return;
        }

        for e in self.props().children.clone() {
            e.update(delta_sec);
        }

        let components: Vec<ComRc<IComponent>> = self
            .components
            .borrow()
            .values()
            .map(|e| e.component.clone())
            .collect();
        for c in components {
            c.on_updating(delta_sec);
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
        self.props_mut().children.push(child);
    }

    fn enabled(&self) -> bool {
        self.props().enabled
    }

    fn set_enabled(&self, enabled: bool) -> () {
        self.props_mut().enabled = enabled;
    }
}
