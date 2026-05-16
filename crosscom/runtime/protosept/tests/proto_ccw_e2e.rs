//! Phase 3 (B1b) end-to-end test for the runtime-typed CCW factory.
//!
//! Validates that `crosscom_protosept::wrap_proto::<I>(handle, data)`
//! produces a Rust `ComRc<I>` whose vtable thunks dispatch back into
//! a script struct conforming to the same `@foreign` proto. Three
//! interfaces exercise different return shapes:
//!
//! - `ICounter` 鈥?`add(int) -> int`, `get() -> int`. Confirms int
//!   args and int returns; ref-count and Drop behaviour.
//! - `IDirectorLike` 鈥?`void activate()` and
//!   `IDirectorLike? update(float)`. Confirms void returns, float
//!   args, and the OptionalForeign return path (both `Null` and
//!   `Some(self)`), including recursive `wrap_proto`.
//! - Plus an unregistered-interface error-path test.

use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::os::raw::{c_float, c_int, c_long};
use std::rc::Rc;
use std::sync::Mutex;

use crosscom::{ComInterface, ComRc, IUnknown, IUnknownVirtualTable};
use crosscom_protosept::{
    register_proto_ccw, wrap_proto, ArgKind, MethodSpec, ProtoSpec, RetKind, RuntimeAccess,
    RuntimeHandle,
};
use p7::interpreter::context::{Context, Data};

// ---------------------------------------------------------------------------
// Test runtime (shared with other tests via duplication; keeping in
// the same file to avoid building a separate shared common module).
// ---------------------------------------------------------------------------

struct TestRuntime {
    ctx: UnsafeCell<Context>,
}

impl TestRuntime {
    fn new(ctx: Context) -> Rc<Self> {
        Rc::new(Self {
            ctx: UnsafeCell::new(ctx),
        })
    }
    fn with_ctx_mut<R>(&self, body: impl FnOnce(&mut Context) -> R) -> R {
        unsafe { body(&mut *self.ctx.get()) }
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

/// Serialises tests in this file; they share static recorder state.
static TEST_LOCK: Mutex<()> = Mutex::new(());

// ---------------------------------------------------------------------------
// ICounter
// ---------------------------------------------------------------------------

const ICOUNTER_UUID: [u8; 16] = [
    0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
];

#[repr(C)]
struct ICounterVtbl {
    iunk: IUnknownVirtualTable,
    add: unsafe extern "system" fn(this: *const *const c_void, delta: c_int) -> c_int,
    get: unsafe extern "system" fn(this: *const *const c_void) -> c_int,
}

#[repr(C)]
struct ICounterInst {
    vtable: *const ICounterVtbl,
}

impl ComInterface for ICounterInst {
    const INTERFACE_ID: [u8; 16] = ICOUNTER_UUID;
}

const COUNTER_SCRIPT: &str = r#"
@foreign(dispatcher="com.invoke", finalizer="com.release",
         type_tag="test.ICounter",
         uuid="11111111-1111-1111-1111-111111111111")
pub proto ICounter {
    fn add(self: ref<ICounter>, delta: int) -> int;
    fn get(self: ref<ICounter>) -> int;
}

@intrinsic(name="test.counter_record")
fn counter_record(seed: int, delta: int);

struct[ICounter] Counter(
    value: int,
) {
    pub fn add(self: ref<Self>, delta: int) -> int {
        // The runtime-typed CCW test cares about marshalling, not
        // mutation; record (value, delta) as a side effect and
        // return the previous value.
        counter_record(self.value, delta);
        self.value
    }
    pub fn get(self: ref<Self>) -> int { self.value }
}

pub fn make_counter(start: int) -> box<ICounter> {
    let c = box(Counter(start));
    c as box<ICounter>
}
"#;

fn register_counter() {
    let _ = register_proto_ccw(ProtoSpec {
        uuid: ICOUNTER_UUID,
        type_tag: "test.ICounter".into(),
        methods: vec![
            MethodSpec {
                name: "add".into(),
                args: vec![ArgKind::Int],
                ret: RetKind::Int,
            },
            MethodSpec {
                name: "get".into(),
                args: vec![],
                ret: RetKind::Int,
            },
        ],
        release_method: None,
        additional_query_uuids: vec![],
    });
    // Idempotent: subsequent calls return Err("already registered"),
    // which is fine for repeated test runs in the same binary.
}

fn invoke_counter_add(counter: &ICounterInst, delta: c_int) -> c_int {
    unsafe {
        let this = counter as *const ICounterInst as *const *const c_void;
        ((*counter.vtable).add)(this, delta)
    }
}

fn invoke_counter_get(counter: &ICounterInst) -> c_int {
    unsafe {
        let this = counter as *const ICounterInst as *const *const c_void;
        ((*counter.vtable).get)(this)
    }
}

static COUNTER_LOG: Mutex<Vec<(i64, i64)>> = Mutex::new(Vec::new());

fn counter_record_host_fn(ctx: &mut Context) -> Result<(), p7::errors::RuntimeError> {
    let frame = ctx.stack_frame_mut()?;
    // Args are pushed in declaration order (value, delta); pop reverses.
    let delta = frame.stack.pop().expect("delta");
    let value = frame.stack.pop().expect("value");
    match (value, delta) {
        (Data::Int(v), Data::Int(d)) => {
            COUNTER_LOG.lock().unwrap().push((v, d));
            Ok(())
        }
        other => panic!("expected (int, int), got {:?}", other),
    }
}

#[test]
fn icounter_roundtrips_add_and_get_through_runtime_typed_ccw() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_counter();
    COUNTER_LOG.lock().unwrap().clear();

    let mut ctx = Context::new();
    ctx.register_host_function("test.counter_record".to_string(), counter_record_host_fn);
    let module = p7::compile(COUNTER_SCRIPT.to_string()).expect("compile");
    ctx.load_module(module);

    ctx.push_function("make_counter", vec![Data::Int(7)]);
    ctx.resume().expect("make_counter ran");
    let counter_data = ctx.stack[0].stack.pop().expect("returned box");

    let runtime = TestRuntime::new(ctx);
    let handle = RuntimeHandle::from_rc(&runtime);

    let counter: ComRc<ICounterInst> =
        wrap_proto(&handle, counter_data).expect("wrap_proto ICounter");

    let inst: &ICounterInst = &counter;
    let prev = invoke_counter_add(inst, 3);
    assert_eq!(prev, 7, "add returns the script's self.value");
    let prev2 = invoke_counter_add(inst, -2);
    assert_eq!(prev2, 7);
    let now = invoke_counter_get(inst);
    assert_eq!(now, 7);
    assert_eq!(
        *COUNTER_LOG.lock().unwrap(),
        vec![(7, 3), (7, -2)],
        "marshalled (value, delta) pairs recorded"
    );

    drop(counter);
    runtime.with_ctx_mut(|ctx| {
        assert!(
            ctx.external_root(0).is_none(),
            "drop must unroot via runtime handle"
        );
    });
}

// ---------------------------------------------------------------------------
// IDirectorLike with OptionalForeign return
// ---------------------------------------------------------------------------

const IDIRLIKE_UUID: [u8; 16] = [
    0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22,
];

#[repr(C)]
struct IDirLikeVtbl {
    iunk: IUnknownVirtualTable,
    activate: unsafe extern "system" fn(this: *const *const c_void),
    /// Returns either a recursive director or null.
    update: unsafe extern "system" fn(this: *const *const c_void, dt: c_float) -> *const c_void,
}

#[repr(C)]
struct IDirLikeInst {
    vtable: *const IDirLikeVtbl,
}

impl ComInterface for IDirLikeInst {
    const INTERFACE_ID: [u8; 16] = IDIRLIKE_UUID;
}

const DIRLIKE_SCRIPT: &str = r#"
@foreign(dispatcher="com.invoke", finalizer="com.release",
         type_tag="test.IDirectorLike",
         uuid="22222222-2222-2222-2222-222222222222")
pub proto IDirectorLike {
    fn activate(self: ref<IDirectorLike>);
    fn update(self: ref<IDirectorLike>, dt: float) -> ?box<IDirectorLike>;
}

@intrinsic(name="test.record_activation")
fn record_activation(seed: int);

struct[IDirectorLike] StubDir(
    seed: int,
    return_self: int,
) {
    pub fn activate(self: ref<Self>) {
        record_activation(self.seed);
    }
    pub fn update(self: ref<Self>, dt: float) -> ?box<IDirectorLike> {
        if self.return_self != 0 {
            // Build a *new* StubDir with return_self=0 so the second
            // activate is observable but the recursion terminates on
            // the next update.
            let next = box(StubDir(self.seed + 100, 0));
            return next as box<IDirectorLike>;
        }
        return null;
    }
}

pub fn make_dir(seed: int, return_self: int) -> box<IDirectorLike> {
    let d = box(StubDir(seed, return_self));
    d as box<IDirectorLike>
}
"#;

static ACTIVATIONS: Mutex<Vec<i64>> = Mutex::new(Vec::new());

fn record_activation_host_fn(ctx: &mut Context) -> Result<(), p7::errors::RuntimeError> {
    let frame = ctx.stack_frame_mut()?;
    let v = frame.stack.pop().expect("seed arg");
    match v {
        Data::Int(n) => {
            ACTIVATIONS.lock().unwrap().push(n);
            Ok(())
        }
        other => panic!("expected int seed, got {:?}", other),
    }
}

fn register_dirlike() {
    let _ = register_proto_ccw(ProtoSpec {
        uuid: IDIRLIKE_UUID,
        type_tag: "test.IDirectorLike".into(),
        methods: vec![
            MethodSpec {
                name: "activate".into(),
                args: vec![],
                ret: RetKind::Void,
            },
            MethodSpec {
                name: "update".into(),
                args: vec![ArgKind::Float],
                ret: RetKind::OptionalForeign {
                    type_tag: "test.IDirectorLike".into(),
                    uuid: IDIRLIKE_UUID,
                },
            },
        ],
        release_method: None,
        additional_query_uuids: vec![],
    });
}

unsafe fn director_activate(d: &IDirLikeInst) {
    let this = d as *const IDirLikeInst as *const *const c_void;
    ((*d.vtable).activate)(this);
}

unsafe fn director_update_returning_raw(d: &IDirLikeInst, dt: f32) -> *const c_void {
    let this = d as *const IDirLikeInst as *const *const c_void;
    ((*d.vtable).update)(this, dt)
}

#[test]
fn idirlike_null_update_returns_null_pointer() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_dirlike();
    ACTIVATIONS.lock().unwrap().clear();

    let mut ctx = Context::new();
    ctx.register_host_function("test.record_activation".to_string(), record_activation_host_fn);
    let module = p7::compile(DIRLIKE_SCRIPT.to_string()).expect("compile");
    ctx.load_module(module);
    ctx.push_function("make_dir", vec![Data::Int(5), Data::Int(0)]);
    ctx.resume().expect("make_dir ran");
    let dir_data = ctx.stack[0].stack.pop().expect("returned box");

    let runtime = TestRuntime::new(ctx);
    let handle = RuntimeHandle::from_rc(&runtime);
    let dir: ComRc<IDirLikeInst> = wrap_proto(&handle, dir_data).expect("wrap_proto");

    unsafe {
        director_activate(&dir);
        let r = director_update_returning_raw(&dir, 0.016);
        assert!(r.is_null(), "update with return_self=0 must yield null");
    }
    assert_eq!(*ACTIVATIONS.lock().unwrap(), vec![5]);
}

#[test]
fn idirlike_some_update_returns_a_new_ccw_thats_independently_invokable() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_dirlike();
    ACTIVATIONS.lock().unwrap().clear();

    let mut ctx = Context::new();
    ctx.register_host_function("test.record_activation".to_string(), record_activation_host_fn);
    let module = p7::compile(DIRLIKE_SCRIPT.to_string()).expect("compile");
    ctx.load_module(module);
    ctx.push_function("make_dir", vec![Data::Int(3), Data::Int(1)]); // return_self=true
    ctx.resume().expect("make_dir ran");
    let dir_data = ctx.stack[0].stack.pop().expect("returned box");

    let runtime = TestRuntime::new(ctx);
    let handle = RuntimeHandle::from_rc(&runtime);
    let dir: ComRc<IDirLikeInst> = wrap_proto(&handle, dir_data).expect("wrap_proto");

    unsafe {
        director_activate(&dir); // record 3
        let raw = director_update_returning_raw(&dir, 0.016);
        assert!(!raw.is_null(), "update with return_self=1 must yield Some");
        // Wrap the returned raw COM pointer as a new ComRc and
        // activate it. The script's StubDir(seed=103, return_self=0)
        // should now record 103.
        let inner: ComRc<IDirLikeInst> = ComRc::<IDirLikeInst>::from_raw_pointer(raw as _);
        director_activate(&inner);
        drop(inner);
    }
    assert_eq!(*ACTIVATIONS.lock().unwrap(), vec![3, 103]);
}

// ---------------------------------------------------------------------------
// Unregistered-interface error path
// ---------------------------------------------------------------------------

const UNREGISTERED_UUID: [u8; 16] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
];

#[repr(C)]
struct INotRegistered {
    vtable: *const c_void,
}
impl ComInterface for INotRegistered {
    const INTERFACE_ID: [u8; 16] = UNREGISTERED_UUID;
}

#[test]
fn wrap_proto_for_unregistered_uuid_errors_loudly() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Use a dangling handle so we never reach the rooting step;
    // wrap_proto should error on the registry lookup before that.
    let handle = RuntimeHandle::dangling();
    let dummy = Data::BoxRef {
        idx: 0,
        generation: 0,
    };
    let err = match wrap_proto::<INotRegistered>(&handle, dummy) {
        Ok(_) => panic!("expected error for unregistered UUID"),
        Err(e) => e,
    };
    let msg = format!("{}", err);
    // Dangling handle is checked before registry lookup; either error
    // is acceptable here as long as the API refuses to produce a
    // ComRc. Prefer to test the registry path with a live handle:
    let _ = msg;

    // Now with a live runtime: registry lookup should fail.
    let mut ctx = Context::new();
    let module = p7::compile(
        r#"pub fn make_box() -> int { 0 }"#.to_string(),
    )
    .expect("compile");
    ctx.load_module(module);
    let runtime = TestRuntime::new(ctx);
    let handle = RuntimeHandle::from_rc(&runtime);
    let dummy = Data::BoxRef {
        idx: 0,
        generation: 0,
    };
    let err2 = match wrap_proto::<INotRegistered>(&handle, dummy) {
        Ok(_) => panic!("expected registry error"),
        Err(e) => e,
    };
    assert!(
        format!("{}", err2).contains("no ProtoSpec registered"),
        "unexpected error: {err2}"
    );
}

// ---------------------------------------------------------------------------
// Release hook (Phase 4 B4): a proto with `release_method` fires the
// named script method on final CCW release.
// ---------------------------------------------------------------------------

const ICLOSEABLE_UUID: [u8; 16] = [
    0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33,
];

#[repr(C)]
struct ICloseableVtbl {
    iunk: IUnknownVirtualTable,
    ping: unsafe extern "system" fn(this: *const *const c_void),
}

#[repr(C)]
struct ICloseableInst {
    vtable: *const ICloseableVtbl,
}

impl ComInterface for ICloseableInst {
    const INTERFACE_ID: [u8; 16] = ICLOSEABLE_UUID;
}

const CLOSEABLE_SCRIPT: &str = r#"
@foreign(dispatcher="com.invoke", finalizer="com.release",
         type_tag="test.ICloseable",
         uuid="33333333-3333-3333-3333-333333333333")
pub proto ICloseable {
    fn ping(self: ref<ICloseable>);
    // `on_close` is declared as part of the proto so `push_proto_method`
    // can find it during the release hook dispatch. This is a Phase 4
    // requirement documented on `ProtoSpec::release_method`.
    fn on_close(self: ref<ICloseable>);
}

@intrinsic(name="test.closeable_record")
fn closeable_record(seed: int, event: int);

struct[ICloseable] Closeable(
    seed: int,
) {
    pub fn ping(self: ref<Self>) {
        closeable_record(self.seed, 1);
    }
    pub fn on_close(self: ref<Self>) {
        closeable_record(self.seed, 2);
    }
}

pub fn make_closeable(seed: int) -> box<ICloseable> {
    let c = box(Closeable(seed));
    c as box<ICloseable>
}
"#;

static CLOSEABLE_LOG: Mutex<Vec<(i64, i64)>> = Mutex::new(Vec::new());

fn closeable_record_host_fn(ctx: &mut Context) -> Result<(), p7::errors::RuntimeError> {
    let frame = ctx.stack_frame_mut()?;
    let event = frame.stack.pop().expect("event");
    let seed = frame.stack.pop().expect("seed");
    match (seed, event) {
        (Data::Int(s), Data::Int(e)) => {
            CLOSEABLE_LOG.lock().unwrap().push((s, e));
            Ok(())
        }
        other => panic!("expected (int, int), got {:?}", other),
    }
}

fn register_closeable() {
    let _ = register_proto_ccw(ProtoSpec {
        uuid: ICLOSEABLE_UUID,
        type_tag: "test.ICloseable".into(),
        methods: vec![MethodSpec {
            name: "ping".into(),
            args: vec![],
            ret: RetKind::Void,
        }],
        release_method: Some("on_close".into()),
        additional_query_uuids: vec![],
    });
}

fn build_closeable_test_ctx(seed: i64) -> (Rc<TestRuntime>, Data) {
    let mut ctx = Context::new();
    ctx.register_host_function(
        "test.closeable_record".to_string(),
        closeable_record_host_fn,
    );
    let module = p7::compile(CLOSEABLE_SCRIPT.to_string()).expect("compile");
    ctx.load_module(module);
    ctx.push_function("make_closeable", vec![Data::Int(seed)]);
    ctx.resume().expect("make_closeable ran");
    let data = ctx.stack[0].stack.pop().expect("returned box");
    (TestRuntime::new(ctx), data)
}

#[test]
fn release_hook_fires_on_final_drop() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_closeable();
    CLOSEABLE_LOG.lock().unwrap().clear();

    let (runtime, data) = build_closeable_test_ctx(42);
    let handle = RuntimeHandle::from_rc(&runtime);
    let inst: ComRc<ICloseableInst> = wrap_proto(&handle, data).expect("wrap_proto");

    // Drop without calling ping; release_method should still fire.
    drop(inst);
    assert_eq!(
        *CLOSEABLE_LOG.lock().unwrap(),
        vec![(42, 2)],
        "on_close fired exactly once on final drop"
    );
    runtime.with_ctx_mut(|ctx| {
        assert!(
            ctx.external_root(0).is_none(),
            "root cleared after release hook + unroot"
        );
    });
}

#[test]
fn release_hook_fires_after_method_invocations() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_closeable();
    CLOSEABLE_LOG.lock().unwrap().clear();

    let (runtime, data) = build_closeable_test_ctx(7);
    let handle = RuntimeHandle::from_rc(&runtime);
    let inst: ComRc<ICloseableInst> = wrap_proto(&handle, data).expect("wrap_proto");

    unsafe {
        let this = &*inst as *const ICloseableInst as *const *const c_void;
        ((*(*inst).vtable).ping)(this);
        ((*(*inst).vtable).ping)(this);
    }
    drop(inst);

    assert_eq!(
        *CLOSEABLE_LOG.lock().unwrap(),
        vec![(7, 1), (7, 1), (7, 2)],
        "ping ping then on_close"
    );
}

#[test]
fn release_hook_silent_noop_after_runtime_drop() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    register_closeable();
    let before = CLOSEABLE_LOG.lock().unwrap().len();

    let (runtime, data) = build_closeable_test_ctx(99);
    let handle = RuntimeHandle::from_rc(&runtime);
    let inst: ComRc<ICloseableInst> = wrap_proto(&handle, data).expect("wrap_proto");

    drop(runtime);
    assert!(handle.is_dangling(), "runtime gone");

    // Final drop must not panic, and must not record an on_close
    // because the runtime can't dispatch it.
    drop(inst);
    let after = CLOSEABLE_LOG.lock().unwrap().clone();
    // The slice past `before` must not contain a (99, _) entry
    // attributable to this test's release hook.
    assert!(
        !after[before..].iter().any(|(s, _)| *s == 99),
        "no on_close should record when runtime is gone; got {after:?}"
    );
}

// Silence unused-c_long warnings on platforms where c_int == c_long.
#[allow(dead_code)]
fn _anchor() {
    let _ = std::mem::size_of::<c_long>();
}
