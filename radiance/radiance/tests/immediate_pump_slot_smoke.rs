//! Phase 5 engine-slot test: confirms the pump-invocation logic
//! that `CoreRadianceEngine::update` runs inside `ui_manager.update`
//! works end-to-end. We test the invocation snippet in isolation
//! since bringing up a real renderer / `UiManager` / `Platform`
//! requires a window and Vulkan.

use std::cell::RefCell;
use std::ffi::c_void;
use std::os::raw::c_long;
use std::rc::Rc;

use crosscom::{ComInterface, ComRc, IUnknown, IUnknownVirtualTable};
use radiance::comdef::IDirector;
use radiance::radiance::ImmediateDirectorPump;

/// Mirrors the snippet in `CoreRadianceEngine::update` that invokes
/// the registered pump after `scene_manager.update`.
fn invoke_immediate_pump(
    pump_slot: &RefCell<Option<Rc<dyn ImmediateDirectorPump>>>,
    director: &Option<ComRc<IDirector>>,
    dt: f32,
) {
    let pump = pump_slot.borrow().clone();
    if let (Some(pump), Some(director)) = (pump, director.as_ref()) {
        pump.pump(director.clone(), dt);
    }
}

struct RecordingPump {
    calls: RefCell<Vec<f32>>,
}

impl RecordingPump {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            calls: RefCell::new(Vec::new()),
        })
    }
}

impl ImmediateDirectorPump for RecordingPump {
    fn pump(&self, _director: ComRc<IDirector>, dt: f32) {
        self.calls.borrow_mut().push(dt);
    }
}

#[repr(C)]
struct StubDirectorVtbl {
    iunk: IUnknownVirtualTable,
    activate: unsafe extern "system" fn(this: *const *const c_void),
    update: unsafe extern "system" fn(
        this: *const *const c_void,
        dt: std::os::raw::c_float,
    ) -> *const c_void,
}

#[repr(C)]
struct StubDirector {
    vtable: *const StubDirectorVtbl,
    refcount: std::cell::Cell<u32>,
}

unsafe extern "system" fn stub_qi(
    this: *const c_void,
    guid: uuid::Uuid,
    retval: &mut *const *const c_void,
) -> c_long {
    let bytes = *guid.as_bytes();
    if bytes == IUnknown::INTERFACE_ID || bytes == IDirector::INTERFACE_ID {
        *retval = this as *const *const c_void;
        stub_addref(*retval);
        0
    } else {
        *retval = std::ptr::null();
        -1
    }
}

unsafe extern "system" fn stub_addref(this: *const *const c_void) -> c_long {
    let d = &*(this as *const StubDirector);
    let prev = d.refcount.get();
    d.refcount.set(prev + 1);
    (prev + 1) as c_long
}

unsafe extern "system" fn stub_release(this: *const *const c_void) -> c_long {
    let d = &*(this as *const StubDirector);
    let prev = d.refcount.get();
    d.refcount.set(prev - 1);
    if prev == 1 {
        drop(Box::from_raw(this as *mut StubDirector));
    }
    (prev - 1) as c_long
}

unsafe extern "system" fn stub_activate(_this: *const *const c_void) {}

unsafe extern "system" fn stub_update(
    _this: *const *const c_void,
    _dt: std::os::raw::c_float,
) -> *const c_void {
    std::ptr::null()
}

static STUB_VTBL: StubDirectorVtbl = StubDirectorVtbl {
    iunk: IUnknownVirtualTable {
        query_interface: stub_qi,
        add_ref: stub_addref,
        release: stub_release,
    },
    activate: stub_activate,
    update: stub_update,
};

fn make_stub_director() -> ComRc<IDirector> {
    let stub = Box::new(StubDirector {
        vtable: &STUB_VTBL as *const StubDirectorVtbl,
        refcount: std::cell::Cell::new(1),
    });
    let raw = Box::into_raw(stub);
    unsafe { ComRc::<IDirector>::from_raw_pointer(raw as *const *const c_void) }
}

#[test]
fn pump_fires_when_slot_and_director_are_both_set() {
    let pump_slot: RefCell<Option<Rc<dyn ImmediateDirectorPump>>> = RefCell::new(None);
    let pump = RecordingPump::new();
    *pump_slot.borrow_mut() = Some(pump.clone());
    let director = Some(make_stub_director());

    invoke_immediate_pump(&pump_slot, &director, 0.016);
    invoke_immediate_pump(&pump_slot, &director, 0.033);

    let calls = pump.calls.borrow().clone();
    assert_eq!(calls.len(), 2);
    assert!((calls[0] - 0.016).abs() < 1e-6);
    assert!((calls[1] - 0.033).abs() < 1e-6);
}

#[test]
fn pump_does_not_fire_when_slot_is_empty() {
    let pump_slot: RefCell<Option<Rc<dyn ImmediateDirectorPump>>> = RefCell::new(None);
    let _director = Some(make_stub_director());
    invoke_immediate_pump(&pump_slot, &_director, 0.016);
}

#[test]
fn pump_does_not_fire_when_no_director() {
    let pump_slot: RefCell<Option<Rc<dyn ImmediateDirectorPump>>> = RefCell::new(None);
    let pump = RecordingPump::new();
    *pump_slot.borrow_mut() = Some(pump.clone());
    invoke_immediate_pump(&pump_slot, &None, 0.016);
    assert!(pump.calls.borrow().is_empty());
}
