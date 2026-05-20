//! Fat-CCW regression: a script struct that conforms to two
//! **unrelated** foreign protos (no IDL inheritance, no
//! `additional_query_uuids` between them) must be QI-able to both
//! interfaces from a single wrap, and dispatch to the correct
//! script method for each. This is the case `additional_query_uuids`
//! alone cannot satisfy — the two interfaces have incompatible
//! vtable layouts, so each needs its own slot.

use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::os::raw::c_int;
use std::rc::Rc;
use std::sync::Mutex;

use crosscom::{ComInterface, ComRc, IUnknownVirtualTable};
use crosscom_protosept::{
    register_proto_ccw, wrap_proto, ArgKind, MethodSpec, ProtoSpec, RetKind, RuntimeAccess,
    RuntimeHandle,
};
use p7::interpreter::context::{Context, Data};

struct TestRuntime {
    ctx: UnsafeCell<Context>,
}

impl TestRuntime {
    fn new(ctx: Context) -> Rc<Self> {
        Rc::new(Self {
            ctx: UnsafeCell::new(ctx),
        })
    }
}

impl RuntimeAccess for TestRuntime {
    fn with_ctx(&self, body: &mut dyn FnMut(&mut Context)) {
        let ctx = unsafe { &mut *self.ctx.get() };
        crosscom_protosept::scope_context(ctx, || {
            let _ = crosscom_protosept::with_context(|c| {
                body(c);
            });
        });
    }
}

static TEST_LOCK: Mutex<()> = Mutex::new(());

const IFOO_UUID: [u8; 16] = [
    0xf0, 0x00, 0xf0, 0x00, 0xf0, 0x00, 0xf0, 0x00, 0xf0, 0x00, 0xf0, 0x00, 0xf0, 0x00, 0xf0, 0x00,
];
const IBAR_UUID: [u8; 16] = [
    0xba, 0x00, 0xba, 0x00, 0xba, 0x00, 0xba, 0x00, 0xba, 0x00, 0xba, 0x00, 0xba, 0x00, 0xba, 0x00,
];

#[repr(C)]
struct IFooVtbl {
    iunk: IUnknownVirtualTable,
    foo: unsafe extern "system" fn(this: *const *const c_void) -> c_int,
}
#[repr(C)]
struct IFooInst {
    vtable: *const IFooVtbl,
}
impl ComInterface for IFooInst {
    const INTERFACE_ID: [u8; 16] = IFOO_UUID;
}

#[repr(C)]
struct IBarVtbl {
    iunk: IUnknownVirtualTable,
    // IBar has a *different* method signature than IFoo to make the
    // point that the two vtables are not layout-compatible.
    bar: unsafe extern "system" fn(this: *const *const c_void, n: c_int) -> c_int,
}
#[repr(C)]
struct IBarInst {
    vtable: *const IBarVtbl,
}
impl ComInterface for IBarInst {
    const INTERFACE_ID: [u8; 16] = IBAR_UUID;
}

const SCRIPT: &str = r#"
@foreign(dispatcher="com.invoke", finalizer="com.release",
         type_tag="test.IFoo",
         uuid="f000f000-f000-f000-f000-f000f000f000")
pub proto IFoo {
    fn foo(self: ref<IFoo>) -> int;
}

@foreign(dispatcher="com.invoke", finalizer="com.release",
         type_tag="test.IBar",
         uuid="ba00ba00-ba00-ba00-ba00-ba00ba00ba00")
pub proto IBar {
    fn bar(self: ref<IBar>, n: int) -> int;
}

@intrinsic(name="test.record")
fn record(tag: int, n: int);

struct[IFoo, IBar] X(seed: int) {
    pub fn foo(self: ref<Self>) -> int {
        record(1, self.seed);
        self.seed + 100
    }
    pub fn bar(self: ref<Self>, n: int) -> int {
        record(2, n);
        self.seed * 10 + n
    }
}

pub fn make() -> box<IFoo> {
    let x = box(X(7));
    x as box<IFoo>
}
"#;

static EVENTS: Mutex<Vec<(i64, i64)>> = Mutex::new(Vec::new());

fn record_host_fn(ctx: &mut Context) -> Result<(), p7::errors::RuntimeError> {
    let frame = ctx.stack_frame_mut()?;
    let n = frame.stack.pop().expect("n");
    let tag = frame.stack.pop().expect("tag");
    match (tag, n) {
        (Data::Int(t), Data::Int(v)) => {
            EVENTS.lock().unwrap().push((t, v));
            Ok(())
        }
        other => panic!("expected (int, int), got {:?}", other),
    }
}

fn register_both() {
    let _ = register_proto_ccw(ProtoSpec {
        uuid: IFOO_UUID,
        type_tag: "test.IFoo".into(),
        methods: vec![MethodSpec {
            name: "foo".into(),
            args: vec![],
            ret: RetKind::Int,
        }],
        additional_query_uuids: vec![],
    });
    let _ = register_proto_ccw(ProtoSpec {
        uuid: IBAR_UUID,
        type_tag: "test.IBar".into(),
        methods: vec![MethodSpec {
            name: "bar".into(),
            args: vec![ArgKind::Int],
            ret: RetKind::Int,
        }],
        additional_query_uuids: vec![],
    });
}

unsafe fn call_foo(inst: &IFooInst) -> c_int {
    let this = inst as *const IFooInst as *const *const c_void;
    ((*inst.vtable).foo)(this)
}

unsafe fn call_bar(inst: &IBarInst, n: c_int) -> c_int {
    let this = inst as *const IBarInst as *const *const c_void;
    ((*inst.vtable).bar)(this, n)
}

unsafe fn qi_to_ibar(foo: &IFooInst) -> Option<ComRc<IBarInst>> {
    let foo_this = foo as *const IFooInst as *const *const c_void;
    let unk = *(foo_this as *const *const IUnknownVirtualTable);
    let guid = uuid::Uuid::from_bytes(IBAR_UUID);
    let mut out: *const *const c_void = std::ptr::null();
    let hr = ((*unk).query_interface)(foo_this as *const c_void, guid, &mut out);
    if hr == 0 && !out.is_null() {
        Some(ComRc::<IBarInst>::from_raw_pointer(out))
    } else {
        None
    }
}

#[test]
fn sibling_interfaces_each_have_their_own_slot_and_dispatch_correctly() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_both();
    EVENTS.lock().unwrap().clear();

    let mut ctx = Context::new();
    ctx.register_host_function("test.record".to_string(), record_host_fn);
    let module = p7::compile(SCRIPT.to_string()).expect("compile");
    ctx.load_module(module);

    ctx.push_function("make", vec![]);
    ctx.resume().expect("make ran");
    let data = ctx.stack[0].stack.pop().expect("returned box");

    let runtime = TestRuntime::new(ctx);
    let handle = RuntimeHandle::from_rc(&runtime);

    // Wrap as IFoo. Without fat CCW this CCW would only know about
    // IFoo; QI to IBar would fail.
    let foo: ComRc<IFooInst> = wrap_proto(&handle, data).expect("wrap_proto IFoo");

    // Dispatch foo through the IFoo slot.
    let foo_ret = unsafe { call_foo(&foo) };
    assert_eq!(foo_ret, 107, "foo returns seed (7) + 100");

    // QI to IBar — this is the fat-CCW feature under test.
    let bar = unsafe { qi_to_ibar(&foo) }
        .expect("QI from IFoo to IBar must succeed via fat-CCW sibling slot");
    let bar_ret = unsafe { call_bar(&bar, 5) };
    assert_eq!(bar_ret, 75, "bar returns seed (7) * 10 + n (5)");

    assert_eq!(
        *EVENTS.lock().unwrap(),
        vec![(1, 7), (2, 5)],
        "foo recorded seed; bar recorded n; both dispatched to the right script method"
    );

    drop(bar);
    drop(foo);
}

#[test]
fn requested_uuid_not_in_conformance_still_satisfied_via_additional_query() {
    // Register IFoo with additional_query_uuids: [IBAR_UUID]. Script
    // declares struct[IFoo] (NOT IBar). Wrap as IBar — the wrap path
    // must pick the IFoo slot (which advertises IBar via QI list)
    // rather than allocating an unbacked IBar slot.
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EVENTS.lock().unwrap().clear();

    // Use fresh UUIDs to avoid colliding with the IFoo registration
    // from the other test in this file (registry is process-global).
    const IINH_PARENT_UUID: [u8; 16] = [
        0xe1, 0x00, 0xe1, 0x00, 0xe1, 0x00, 0xe1, 0x00, 0xe1, 0x00, 0xe1, 0x00, 0xe1, 0x00, 0xe1,
        0x00,
    ];
    const IINH_CHILD_UUID: [u8; 16] = [
        0xe2, 0x00, 0xe2, 0x00, 0xe2, 0x00, 0xe2, 0x00, 0xe2, 0x00, 0xe2, 0x00, 0xe2, 0x00, 0xe2,
        0x00,
    ];

    let _ = register_proto_ccw(ProtoSpec {
        uuid: IINH_CHILD_UUID,
        type_tag: "test.IInhChild".into(),
        methods: vec![MethodSpec {
            name: "do_it".into(),
            args: vec![],
            ret: RetKind::Int,
        }],
        // Child advertises Parent via QI — exactly the
        // IImmediateDirector → IDirector relationship.
        additional_query_uuids: vec![IINH_PARENT_UUID],
    });

    const INH_SCRIPT: &str = r#"
@foreign(dispatcher="com.invoke", finalizer="com.release",
         type_tag="test.IInhChild",
         uuid="e200e200-e200-e200-e200-e200e200e200")
pub proto IInhChild {
    fn do_it(self: ref<IInhChild>) -> int;
}

struct[IInhChild] Z(seed: int) {
    pub fn do_it(self: ref<Self>) -> int { self.seed * 2 }
}

pub fn make() -> box<IInhChild> {
    let z = box(Z(11));
    z as box<IInhChild>
}
"#;
    // Note the foreign uuid in the script is the *child*'s uuid (e1...).
    let _ = INH_SCRIPT; // referenced below in compile().

    let mut ctx = Context::new();
    let module = p7::compile(INH_SCRIPT.to_string()).expect("compile inh");
    ctx.load_module(module);
    ctx.push_function("make", vec![]);
    ctx.resume().expect("make ran");
    let data = ctx.stack[0].stack.pop().expect("returned box");

    let runtime = TestRuntime::new(ctx);
    let handle = RuntimeHandle::from_rc(&runtime);

    // Wrap requesting the *parent* UUID. The script only conforms
    // to the child, but the child's spec lists the parent in
    // additional_query_uuids, so the wrap should pick the child's
    // slot (whose vtable layout is compatible with the parent).
    let raw = crosscom_protosept::wrap_proto_unknown(&handle, data, IINH_PARENT_UUID)
        .expect("wrap_proto_unknown via additional_query_uuids");
    drop(raw);
}
