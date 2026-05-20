use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::os::raw::c_long;
use std::rc::Rc;

use crosscom::{ComInterface, ComRc, IUnknown, IUnknownVirtualTable};
use radiance::comdef::{IComponent, IComponentVirtualTable, IEntity, IScene};
use radiance::scene::{CoreEntity, CoreScene};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Loading(&'static str),
    Updating(&'static str),
    Unloading(&'static str),
    Marker(&'static str),
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
    hooks: ComponentHooks,
) -> ComRc<IComponent> {
    let component = Box::new(RecordingComponent {
        vtable: &RECORDING_COMPONENT_VTBL as *const StubComponentVtbl
            as *const IComponentVirtualTable,
        refcount: Cell::new(1),
        state: Rc::new(RecordingState {
            name,
            events,
            hooks: RefCell::new(hooks),
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

fn count(events: &[Event], needle: Event) -> usize {
    events.iter().filter(|event| **event == needle).count()
}

fn position(events: &[Event], needle: Event) -> usize {
    events
        .iter()
        .position(|event| *event == needle)
        .expect("event missing")
}

#[test]
fn entity_update_add_component_during_dispatch_defers_first_on_updating() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let entity: ComRc<IEntity> = CoreEntity::create("entity".into(), true);
    let uuid_a = test_uuid(1);
    let uuid_b = test_uuid(2);
    let added = Rc::new(Cell::new(false));

    entity.load();

    let component_b = make_recording_component("B", events.clone(), ComponentHooks::default());
    let component_a = make_recording_component(
        "A",
        events.clone(),
        ComponentHooks {
            on_updating: Some(Box::new({
                let entity = entity.clone();
                let component_b = component_b.clone();
                let added = added.clone();
                move |_| {
                    if !added.replace(true) {
                        entity.add_component(uuid_b, component_b.clone());
                    }
                }
            })),
            ..ComponentHooks::default()
        },
    );

    entity.add_component(uuid_a, component_a);
    entity.update(0.016);

    let after_first = snapshot(&events);
    assert_eq!(count(&after_first, Event::Loading("B")), 1);
    assert_eq!(count(&after_first, Event::Updating("B")), 0);

    entity.update(0.016);

    let after_second = snapshot(&events);
    assert_eq!(count(&after_second, Event::Updating("B")), 1);
}

#[test]
fn entity_update_remove_component_during_dispatch_is_deferred_but_unload_is_synchronous() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let entity: ComRc<IEntity> = CoreEntity::create("entity".into(), true);
    let uuid_a = test_uuid(3);
    let uuid_b = test_uuid(4);

    entity.load();

    let component_b = make_recording_component("B", events.clone(), ComponentHooks::default());
    let component_a = make_recording_component(
        "A",
        events.clone(),
        ComponentHooks {
            on_updating: Some(Box::new({
                let entity = entity.clone();
                let events = events.clone();
                move |_| {
                    events.borrow_mut().push(Event::Marker("before-remove"));
                    let removed = entity.remove_component(uuid_b);
                    assert!(removed.is_some());
                    events.borrow_mut().push(Event::Marker("after-remove"));
                }
            })),
            ..ComponentHooks::default()
        },
    );

    entity.add_component(uuid_a, component_a);
    entity.add_component(uuid_b, component_b);
    entity.update(0.016);

    let recorded = snapshot(&events);
    assert_eq!(count(&recorded, Event::Unloading("B")), 1);
    assert_eq!(count(&recorded, Event::Updating("B")), 0);
    assert!(
        position(&recorded, Event::Marker("before-remove"))
            < position(&recorded, Event::Unloading("B"))
    );
    assert!(
        position(&recorded, Event::Unloading("B"))
            < position(&recorded, Event::Marker("after-remove"))
    );
    assert!(entity.get_component(uuid_b).is_none());
}

#[test]
fn scene_add_entity_during_dispatch_is_deferred() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let scene: ComRc<IScene> = CoreScene::create();
    let e1: ComRc<IEntity> = CoreEntity::create("e1".into(), true);
    let e2: ComRc<IEntity> = CoreEntity::create("e2".into(), true);
    let uuid_a = test_uuid(5);
    let uuid_b = test_uuid(6);
    let added = Rc::new(Cell::new(false));

    let component_b = make_recording_component("B", events.clone(), ComponentHooks::default());
    e2.add_component(uuid_b, component_b);

    let component_a = make_recording_component(
        "A",
        events.clone(),
        ComponentHooks {
            on_updating: Some(Box::new({
                let scene = scene.clone();
                let e2 = e2.clone();
                let added = added.clone();
                move |_| {
                    if !added.replace(true) {
                        scene.add_entity(e2.clone());
                    }
                }
            })),
            ..ComponentHooks::default()
        },
    );
    e1.add_component(uuid_a, component_a);

    scene.add_entity(e1);
    scene.load();
    scene.update(0.016);

    let after_first = snapshot(&events);
    assert_eq!(count(&after_first, Event::Loading("B")), 1);
    assert_eq!(count(&after_first, Event::Updating("B")), 0);

    scene.update(0.016);

    let after_second = snapshot(&events);
    assert_eq!(count(&after_second, Event::Updating("B")), 1);
}

#[test]
fn entity_same_uuid_re_add_silently_overwrites_no_on_unloading_fired() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let entity: ComRc<IEntity> = CoreEntity::create("entity".into(), true);
    let uuid_x = test_uuid(7);

    entity.load();

    let component_a = make_recording_component("A", events.clone(), ComponentHooks::default());
    let component_b = make_recording_component("B", events.clone(), ComponentHooks::default());

    entity.add_component(uuid_x, component_a);
    events.borrow_mut().clear();

    entity.add_component(uuid_x, component_b.clone());

    let recorded = snapshot(&events);
    assert_eq!(count(&recorded, Event::Unloading("A")), 0);
    assert_eq!(count(&recorded, Event::Loading("B")), 1);
    let current = entity
        .get_component(uuid_x)
        .expect("replacement component missing");
    assert_eq!(current.ptr_value(), component_b.ptr_value());
}

#[test]
fn drain_pending_reaches_fixed_point_when_queued_mutation_queues_another() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let entity: ComRc<IEntity> = CoreEntity::create("entity".into(), true);
    let uuid_a = test_uuid(8);
    let uuid_b = test_uuid(9);
    let uuid_c = test_uuid(10);
    let added_b = Rc::new(Cell::new(false));
    let added_c = Rc::new(Cell::new(false));
    let trigger_count = Rc::new(Cell::new(0u32));

    entity.load();

    let component_c = make_recording_component("C", events.clone(), ComponentHooks::default());
    let component_b = make_recording_component(
        "B",
        events.clone(),
        ComponentHooks {
            on_updating: Some(Box::new({
                let entity = entity.clone();
                let component_c = component_c.clone();
                let added_c = added_c.clone();
                move |_| {
                    if !added_c.replace(true) {
                        entity.add_component(uuid_c, component_c.clone());
                    }
                }
            })),
            ..ComponentHooks::default()
        },
    );
    let component_a = make_recording_component(
        "A",
        events.clone(),
        ComponentHooks {
            on_updating: Some(Box::new({
                let entity = entity.clone();
                let component_b = component_b.clone();
                let added_b = added_b.clone();
                let trigger_count = trigger_count.clone();
                move |_| {
                    if !added_b.replace(true) {
                        trigger_count.set(trigger_count.get() + 1);
                        entity.add_component(uuid_b, component_b.clone());
                    }
                }
            })),
            ..ComponentHooks::default()
        },
    );

    entity.add_component(uuid_a, component_a);

    entity.update(0.016);
    entity.update(0.016);
    entity.update(0.016);

    let recorded = snapshot(&events);
    assert_eq!(trigger_count.get(), 1);
    assert_eq!(count(&recorded, Event::Loading("A")), 1);
    assert_eq!(count(&recorded, Event::Loading("B")), 1);
    assert_eq!(count(&recorded, Event::Loading("C")), 1);
    assert!(entity.get_component(uuid_b).is_some());
    assert!(entity.get_component(uuid_c).is_some());
}

#[test]
fn drain_pending_applies_nested_adds_from_on_loading() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let entity: ComRc<IEntity> = CoreEntity::create("entity".into(), true);
    let uuid_a = test_uuid(11);
    let uuid_b = test_uuid(12);
    let uuid_c = test_uuid(13);
    let added_b = Rc::new(Cell::new(false));

    entity.load();

    let component_c = make_recording_component("C", events.clone(), ComponentHooks::default());
    let component_b = make_recording_component(
        "B",
        events.clone(),
        ComponentHooks {
            on_loading: Some(Box::new({
                let entity = entity.clone();
                let component_c = component_c.clone();
                move || {
                    entity.add_component(uuid_c, component_c.clone());
                }
            })),
            ..ComponentHooks::default()
        },
    );
    let component_a = make_recording_component(
        "A",
        events.clone(),
        ComponentHooks {
            on_updating: Some(Box::new({
                let entity = entity.clone();
                let component_b = component_b.clone();
                let added_b = added_b.clone();
                move |_| {
                    if !added_b.replace(true) {
                        entity.add_component(uuid_b, component_b.clone());
                    }
                }
            })),
            ..ComponentHooks::default()
        },
    );

    entity.add_component(uuid_a, component_a);
    entity.update(0.016);

    let recorded = snapshot(&events);
    assert_eq!(count(&recorded, Event::Loading("B")), 1);
    assert_eq!(count(&recorded, Event::Loading("C")), 1);
    assert!(entity.get_component(uuid_b).is_some());
    assert!(entity.get_component(uuid_c).is_some());
}
