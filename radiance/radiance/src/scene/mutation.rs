//! Shared component-mutation infrastructure for scene containers.
//!
//! `ComponentBag` owns the component map and preserves the existing lifecycle
//! contract used by `CoreEntity` and `CoreScene`: container code decides when
//! lifecycle hooks fire. `load_all` and `drain_loaded_for_unload` are the only
//! helpers here that invoke hooks, and `insert` deliberately does not fire
//! `on_unloading` for a displaced entry so the pre-existing same-UUID overwrite
//! quirk remains intact.

use std::cell::{Cell, RefCell};

use crosscom::ComRc;
use indexmap::IndexMap;
use uuid::Uuid;

use crate::comdef::IComponent;

pub(crate) struct ComponentEntry {
    pub(crate) component: ComRc<IComponent>,
    pub(crate) loaded: bool,
    pub(crate) pending_removal: bool,
}

pub(crate) struct ComponentBag {
    components: RefCell<IndexMap<Uuid, ComponentEntry>>,
}

impl ComponentBag {
    pub(crate) fn new() -> Self {
        Self {
            components: RefCell::new(IndexMap::new()),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.components.borrow().is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.components.borrow().len()
    }

    pub(crate) fn get(&self, uuid: Uuid) -> Option<ComRc<IComponent>> {
        self.components.borrow().get(&uuid).and_then(|entry| {
            if entry.pending_removal {
                None
            } else {
                Some(entry.component.clone())
            }
        })
    }

    pub(crate) fn insert(&self, uuid: Uuid, component: ComRc<IComponent>, loaded: bool) {
        self.components.borrow_mut().insert(
            uuid,
            ComponentEntry {
                component,
                loaded,
                pending_removal: false,
            },
        );
    }

    pub(crate) fn shift_remove(&self, uuid: Uuid) -> Option<ComponentEntry> {
        self.components.borrow_mut().shift_remove(&uuid)
    }

    pub(crate) fn mark_pending_removal(&self, uuid: Uuid) -> Option<(ComRc<IComponent>, bool)> {
        let mut components = self.components.borrow_mut();
        let entry = components.get_mut(&uuid)?;
        if entry.pending_removal {
            return None;
        }

        let was_loaded = entry.loaded;
        entry.loaded = false;
        entry.pending_removal = true;
        Some((entry.component.clone(), was_loaded))
    }

    pub(crate) fn load_all(&self) {
        let uuids: Vec<Uuid> = self.components.borrow().keys().copied().collect();
        for uuid in uuids {
            let component = {
                let mut components = self.components.borrow_mut();
                let entry = match components.get_mut(&uuid) {
                    Some(entry) => entry,
                    None => continue,
                };
                if entry.loaded || entry.pending_removal {
                    continue;
                }
                entry.loaded = true;
                entry.component.clone()
            };
            component.on_loading();
        }
    }

    pub(crate) fn drain_loaded_for_unload(&self) -> Vec<ComRc<IComponent>> {
        self.components
            .borrow_mut()
            .drain(..)
            .filter_map(|(_, entry)| {
                if entry.loaded {
                    Some(entry.component)
                } else {
                    None
                }
            })
            .collect()
    }

    pub(crate) fn dispatch_each<F: FnMut(ComRc<IComponent>)>(&self, mut f: F) {
        let len = self.len();
        for i in 0..len {
            let component = {
                let components = self.components.borrow();
                let (_uuid, entry) = match components.get_index(i) {
                    Some(entry) => entry,
                    None => continue,
                };
                if entry.pending_removal {
                    continue;
                }
                entry.component.clone()
            };
            f(component);
        }
    }
}

pub(crate) struct IterGuard<'a> {
    depth: &'a Cell<u32>,
}

impl<'a> IterGuard<'a> {
    pub(crate) fn new(depth: &'a Cell<u32>) -> Self {
        depth.set(depth.get() + 1);
        Self { depth }
    }
}

impl<'a> Drop for IterGuard<'a> {
    fn drop(&mut self) {
        self.depth.set(self.depth.get() - 1);
    }
}

pub(crate) struct MutationQueue<T> {
    depth: Cell<u32>,
    pending: RefCell<Vec<T>>,
}

impl<T> MutationQueue<T> {
    pub(crate) fn new() -> Self {
        Self {
            depth: Cell::new(0),
            pending: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn is_iterating(&self) -> bool {
        self.depth.get() > 0
    }

    pub(crate) fn iter_guard(&self) -> IterGuard<'_> {
        IterGuard::new(&self.depth)
    }

    pub(crate) fn enqueue(&self, change: T) {
        self.pending.borrow_mut().push(change);
    }

    pub(crate) fn drain<F: FnMut(T)>(&self, mut apply: F) {
        loop {
            let batch: Vec<T> = self.pending.borrow_mut().drain(..).collect();
            if batch.is_empty() {
                break;
            }

            for change in batch {
                apply(change);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::ffi::c_void;
    use std::os::raw::c_long;
    use std::rc::Rc;

    use crosscom::{ComInterface, ComRc, IUnknown, IUnknownVirtualTable};

    use super::{ComponentBag, MutationQueue};
    use crate::comdef::{IComponent, IComponentVirtualTable};
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Event {
        Loading(&'static str),
        Updating(&'static str),
        Unloading(&'static str),
    }

    #[derive(Default)]
    struct ComponentHooks {
        on_loading: Option<Box<dyn FnMut()>>,
        on_updating: Option<Box<dyn FnMut(f32)>>,
        on_unloading: Option<Box<dyn FnMut()>>,
    }

    struct RecordingState {
        name: &'static str,
        events: Rc<RefCell<Vec<Event>>>,
        hooks: RefCell<ComponentHooks>,
    }

    #[repr(C)]
    struct RecordingComponent {
        vtable: *const IComponentVirtualTable,
        refcount: Cell<u32>,
        state: Rc<RecordingState>,
    }

    #[repr(C)]
    struct StubComponentVtbl {
        iunk: IUnknownVirtualTable,
        on_loading: unsafe extern "system" fn(this: *const *const c_void),
        on_updating:
            unsafe extern "system" fn(this: *const *const c_void, delta_sec: std::os::raw::c_float),
        on_unloading: unsafe extern "system" fn(this: *const *const c_void),
    }

    unsafe extern "system" fn component_qi(
        this: *const c_void,
        guid: Uuid,
        retval: &mut *const *const c_void,
    ) -> c_long {
        let bytes = *guid.as_bytes();
        if bytes == IUnknown::INTERFACE_ID || bytes == IComponent::INTERFACE_ID {
            *retval = this as *const *const c_void;
            component_addref(*retval);
            0
        } else {
            *retval = std::ptr::null();
            -1
        }
    }

    unsafe extern "system" fn component_addref(this: *const *const c_void) -> c_long {
        let component = &*(this as *const RecordingComponent);
        let prev = component.refcount.get();
        component.refcount.set(prev + 1);
        (prev + 1) as c_long
    }

    unsafe extern "system" fn component_release(this: *const *const c_void) -> c_long {
        let component = &*(this as *const RecordingComponent);
        let prev = component.refcount.get();
        component.refcount.set(prev - 1);
        if prev == 1 {
            drop(Box::from_raw(this as *mut RecordingComponent));
        }
        (prev - 1) as c_long
    }

    unsafe extern "system" fn component_on_loading(this: *const *const c_void) {
        let component = &*(this as *const RecordingComponent);
        component
            .state
            .events
            .borrow_mut()
            .push(Event::Loading(component.state.name));
        if let Some(hook) = component.state.hooks.borrow_mut().on_loading.as_mut() {
            hook();
        }
    }

    unsafe extern "system" fn component_on_updating(this: *const *const c_void, delta_sec: f32) {
        let component = &*(this as *const RecordingComponent);
        component
            .state
            .events
            .borrow_mut()
            .push(Event::Updating(component.state.name));
        if let Some(hook) = component.state.hooks.borrow_mut().on_updating.as_mut() {
            hook(delta_sec);
        }
    }

    unsafe extern "system" fn component_on_unloading(this: *const *const c_void) {
        let component = &*(this as *const RecordingComponent);
        component
            .state
            .events
            .borrow_mut()
            .push(Event::Unloading(component.state.name));
        if let Some(hook) = component.state.hooks.borrow_mut().on_unloading.as_mut() {
            hook();
        }
    }

    static RECORDING_COMPONENT_VTBL: StubComponentVtbl = StubComponentVtbl {
        iunk: IUnknownVirtualTable {
            query_interface: component_qi,
            add_ref: component_addref,
            release: component_release,
        },
        on_loading: component_on_loading,
        on_updating: component_on_updating,
        on_unloading: component_on_unloading,
    };

    fn make_recording_component(
        name: &'static str,
        events: Rc<RefCell<Vec<Event>>>,
    ) -> ComRc<IComponent> {
        let component = Box::new(RecordingComponent {
            vtable: &RECORDING_COMPONENT_VTBL as *const StubComponentVtbl
                as *const IComponentVirtualTable,
            refcount: Cell::new(1),
            state: Rc::new(RecordingState {
                name,
                events,
                hooks: RefCell::new(ComponentHooks::default()),
            }),
        });
        let raw = Box::into_raw(component);
        unsafe { ComRc::<IComponent>::from_raw_pointer(raw as *const *const c_void) }
    }

    fn test_uuid(tag: u8) -> Uuid {
        let mut bytes = [0; 16];
        bytes[15] = tag;
        Uuid::from_bytes(bytes)
    }

    fn snapshot(events: &Rc<RefCell<Vec<Event>>>) -> Vec<Event> {
        events.borrow().clone()
    }

    #[test]
    fn iter_guard_increments_and_decrements_depth_atomically() {
        let queue = MutationQueue::<()>::new();
        assert_eq!(queue.depth.get(), 0);
        assert!(!queue.is_iterating());

        let outer = queue.iter_guard();
        assert_eq!(queue.depth.get(), 1);
        assert!(queue.is_iterating());

        {
            let inner = queue.iter_guard();
            assert_eq!(queue.depth.get(), 2);
            assert!(queue.is_iterating());
            drop(inner);
        }

        assert_eq!(queue.depth.get(), 1);
        assert!(queue.is_iterating());
        drop(outer);

        assert_eq!(queue.depth.get(), 0);
        assert!(!queue.is_iterating());
    }

    #[test]
    fn mutation_queue_drain_reaches_fixed_point_when_apply_requeues() {
        let queue = MutationQueue::new();
        let applied = Rc::new(RefCell::new(Vec::new()));
        queue.enqueue(1);

        queue.drain(|change| {
            applied.borrow_mut().push(change);
            if change == 1 {
                queue.enqueue(2);
                queue.enqueue(3);
            }
        });

        assert_eq!(*applied.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn mutation_queue_drain_empty_is_noop() {
        let queue = MutationQueue::<i32>::new();
        let calls = Cell::new(0);

        queue.drain(|_| calls.set(calls.get() + 1));

        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn component_bag_dispatch_each_skips_pending_removal() {
        let bag = ComponentBag::new();
        let events = Rc::new(RefCell::new(Vec::new()));
        bag.insert(
            test_uuid(1),
            make_recording_component("keep", events.clone()),
            true,
        );
        bag.insert(
            test_uuid(2),
            make_recording_component("skip", events.clone()),
            true,
        );
        bag.mark_pending_removal(test_uuid(2));

        bag.dispatch_each(|component| component.on_updating(0.0));

        assert_eq!(snapshot(&events), vec![Event::Updating("keep")]);
    }

    #[test]
    fn component_bag_load_all_fires_on_loading_once_per_unloaded_entry() {
        let bag = ComponentBag::new();
        let events = Rc::new(RefCell::new(Vec::new()));
        let first = test_uuid(1);
        let second = test_uuid(2);
        bag.insert(
            first,
            make_recording_component("first", events.clone()),
            false,
        );
        bag.insert(
            second,
            make_recording_component("second", events.clone()),
            false,
        );

        bag.load_all();

        assert_eq!(
            snapshot(&events),
            vec![Event::Loading("first"), Event::Loading("second")]
        );
        {
            let components = bag.components.borrow();
            assert!(components.get(&first).unwrap().loaded);
            assert!(components.get(&second).unwrap().loaded);
        }

        bag.load_all();

        assert_eq!(
            snapshot(&events),
            vec![Event::Loading("first"), Event::Loading("second")]
        );
    }

    #[test]
    fn component_bag_drain_loaded_for_unload_returns_only_loaded_entries() {
        let bag = ComponentBag::new();
        let events = Rc::new(RefCell::new(Vec::new()));
        bag.insert(
            test_uuid(1),
            make_recording_component("loaded", events.clone()),
            true,
        );
        bag.insert(
            test_uuid(2),
            make_recording_component("unloaded", events.clone()),
            false,
        );

        let unloading = bag.drain_loaded_for_unload();

        assert_eq!(unloading.len(), 1);
        assert_eq!(bag.len(), 0);
        unloading[0].on_unloading();
        assert_eq!(snapshot(&events), vec![Event::Unloading("loaded")]);
    }

    #[test]
    fn component_bag_insert_overwrites_silently_no_on_unloading() {
        let bag = ComponentBag::new();
        let events = Rc::new(RefCell::new(Vec::new()));
        let uuid = test_uuid(1);
        bag.insert(uuid, make_recording_component("a", events.clone()), true);
        bag.insert(uuid, make_recording_component("b", events.clone()), true);

        let component = bag
            .get(uuid)
            .expect("component should exist after overwrite");
        component.on_unloading();

        assert_eq!(snapshot(&events), vec![Event::Unloading("b")]);
    }

    #[test]
    fn component_bag_mark_pending_removal_returns_was_loaded_and_clears_loaded_flag() {
        let bag = ComponentBag::new();
        let events = Rc::new(RefCell::new(Vec::new()));
        let uuid = test_uuid(1);
        bag.insert(uuid, make_recording_component("marked", events), true);

        let (_component, was_loaded) = bag
            .mark_pending_removal(uuid)
            .expect("first mark_pending_removal should succeed");

        assert!(was_loaded);
        assert!(bag.mark_pending_removal(uuid).is_none());
        assert!(bag.get(uuid).is_none());
        assert_eq!(bag.len(), 1);
        let components = bag.components.borrow();
        let entry = components.get(&uuid).expect("entry should remain present");
        assert!(!entry.loaded);
        assert!(entry.pending_removal);
    }
}
