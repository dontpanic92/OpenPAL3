//! End-to-end test for the generic `com.invoke` / `com.release`
//! dispatcher in `crosscom-protosept`.
//!
//! Builds a fake `IUnknown`-derived interface ("ICounter") with a
//! hand-rolled `#[repr(C)]` vtable, pushes a script that calls one of
//! its methods through the `@foreign` proto path, and asserts the value
//! returned by the vtable round-trips back into the protosept stack.

use std::ffi::c_void;
use std::os::raw::c_long;

use crosscom::{ComInterface, IUnknown, IUnknownVirtualTable};
use crosscom_protosept::{install_com_dispatcher, scope, ComObjectTable, HostServices};
use p7::interpreter::context::{Context, Data};

const COUNTER_UUID_BYTES: [u8; 16] = [
    0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
];

#[repr(C)]
struct ICounterVtbl {
    iunk: IUnknownVirtualTable,
    add: unsafe extern "system" fn(this: *const *const c_void, delta: c_long) -> c_long,
}

unsafe extern "system" fn fake_qi(
    this: *const c_void,
    guid: uuid::Uuid,
    retval: &mut *const *const c_void,
) -> c_long {
    let bytes = *guid.as_bytes();
    if bytes == IUnknown::INTERFACE_ID || bytes == COUNTER_UUID_BYTES {
        *retval = this as *const *const c_void;
        unsafe { fake_addref(*retval) };
        0
    } else {
        *retval = std::ptr::null();
        -1
    }
}

unsafe extern "system" fn fake_addref(this: *const *const c_void) -> c_long {
    let header = unsafe { &mut *(this as *mut FakeCounter) };
    header.refcount += 1;
    header.refcount as c_long
}

unsafe extern "system" fn fake_release(this: *const *const c_void) -> c_long {
    let header = unsafe { &mut *(this as *mut FakeCounter) };
    header.refcount -= 1;
    header.refcount as c_long
}

unsafe extern "system" fn fake_add(this: *const *const c_void, delta: c_long) -> c_long {
    let header = unsafe { &mut *(this as *mut FakeCounter) };
    let prev = header.value;
    header.value += delta;
    prev
}

#[repr(C)]
struct FakeCounter {
    vtable: *const ICounterVtbl,
    refcount: usize,
    value: c_long,
}

static COUNTER_VTBL: ICounterVtbl = ICounterVtbl {
    iunk: IUnknownVirtualTable {
        query_interface: fake_qi,
        add_ref: fake_addref,
        release: fake_release,
    },
    add: fake_add,
};

struct TestServices {
    com: ComObjectTable,
}

impl HostServices for TestServices {
    fn com_table_mut(&mut self) -> &mut ComObjectTable {
        &mut self.com
    }
}

const SOURCE: &str = r#"
@foreign(dispatcher="com.invoke", finalizer="com.release",
         type_tag="test.ICounter",
         uuid="deadbeef-cafe-babe-0000-000000000001")
pub proto ICounter {
    fn add(self: ref<ICounter>, delta: int) -> int;
}

@intrinsic(name="test.make_counter")
pub fn make_counter() -> box<ICounter>;

pub fn run() -> int {
    let c: box<ICounter> = make_counter();
    let _x: int = c.add(10);
    let _y: int = c.add(5);
    c.add(0)
}
"#;

fn host_make_counter(ctx: &mut Context) -> Result<(), p7::errors::RuntimeError> {
    let counter = Box::new(FakeCounter {
        vtable: &COUNTER_VTBL as *const ICounterVtbl,
        refcount: 1,
        value: 0,
    });
    let raw = Box::into_raw(counter) as *const *const c_void;
    let unk = unsafe { crosscom::ComRc::<IUnknown>::from_raw_pointer(raw) };
    let handle = crosscom_protosept::with_services(|s| s.com_table_mut().intern_unknown(unk))
        .map_err(|e| p7::errors::RuntimeError::Other(format!("with_services: {}", e)))?;
    ctx.push_foreign("test.ICounter", handle)
}

#[test]
fn dispatcher_roundtrips_long_arg_and_return_through_real_vtable() {
    let module = p7::compile(SOURCE.to_string()).expect("compile");
    let mut ctx = Context::new();
    install_com_dispatcher(&mut ctx);
    ctx.register_host_function("test.make_counter".to_string(), host_make_counter);

    ctx.load_module(module);

    let mut services = TestServices {
        com: ComObjectTable::new(),
    };
    scope(&mut services, || {
        ctx.push_function("run", Vec::new());
        ctx.resume().expect("run");

        let result = ctx.stack[0].stack.pop().expect("result");
        assert_eq!(result, Data::Int(15));

        ctx.collect_garbage();
    });
}
