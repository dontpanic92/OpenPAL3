//! Lifetime tests for `wrap_action`: a `ComRc<IAction>` returned by
//! `crosscom_protosept::wrap_action` must behave correctly even when
//! the surrounding `scope_context` is gone (the v1 lifetime caveat
//! that Phase B3 in `generated/director.md` set out to lift), and
//! must also tolerate the runtime itself being dropped before the
//! `ComRc` is.

use std::cell::UnsafeCell;
use std::rc::Rc;
use std::sync::Mutex;

use crosscom_protosept::{wrap_action, RuntimeAccess, RuntimeHandle};
use p7::interpreter::context::{Context, Data};

const SOURCE: &str = r#"
@foreign(dispatcher="action.invoke", type_tag="test.IAction")
pub proto IAction {
    fn invoke(self: ref<IAction>);
}

@intrinsic(name="recorder.set")
fn record_invocation(seed: int);

struct[IAction] Recorder(
    seed: int,
) {
    pub fn invoke(self: ref<Self>) {
        record_invocation(self.seed);
    }
}

pub fn make_recorder(seed: int) -> box<IAction> {
    let r = box(Recorder(seed));
    r as box<IAction>
}
"#;

static RECORDED: Mutex<Vec<i64>> = Mutex::new(Vec::new());
/// Serialize all tests in this file because they share `RECORDED`
/// and reuse seed 42.
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn recorder_set_host_fn(ctx: &mut Context) -> Result<(), p7::errors::RuntimeError> {
    let frame = ctx.stack_frame_mut()?;
    let v = frame.stack.pop().expect("seed arg");
    match v {
        Data::Int(n) => {
            RECORDED.lock().unwrap().push(n);
            Ok(())
        }
        other => panic!("expected int seed, got {:?}", other),
    }
}

/// Minimal `RuntimeAccess` implementor used across the lifetime
/// tests. Owns the interpreter `Context` behind an `UnsafeCell` so a
/// `Weak<dyn RuntimeAccess>` derived from an `Rc<TestRuntime>` can
/// reach back through it. Linear access pattern; not re-entrant.
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
        // SAFETY: single-threaded; tests issue non-overlapping borrows.
        unsafe { body(&mut *self.ctx.get()) }
    }
}

impl RuntimeAccess for TestRuntime {
    fn with_ctx(&self, body: &mut dyn FnMut(&mut Context)) {
        // SAFETY: as above.
        let ctx = unsafe { &mut *self.ctx.get() };
        crosscom_protosept::scope_context(ctx, || {
            let _ = crosscom_protosept::with_context(|c| {
                body(c);
            });
        });
    }
}

fn build_runtime_with_recorder() -> (Rc<TestRuntime>, Data) {
    let module = p7::compile(SOURCE.to_string()).expect("compile");
    let mut ctx = Context::new();
    ctx.register_host_function("recorder.set".to_string(), recorder_set_host_fn);
    ctx.load_module(module);

    ctx.push_function("make_recorder", vec![Data::Int(42)]);
    ctx.resume().expect("make_recorder ran");
    let action_data = ctx.stack[0].stack.pop().expect("returned box");

    let runtime = TestRuntime::new(ctx);
    (runtime, action_data)
}

/// (1) The ComRc may be invoked from any caller, with no outer
/// `scope_context` on the call stack. The CCW captures the runtime
/// handle on construction and re-enters via it on every invoke.
#[test]
fn comrc_invokes_without_outer_scope_context() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    RECORDED.lock().unwrap().clear();
    let (runtime, action_data) = build_runtime_with_recorder();
    let handle = RuntimeHandle::from_rc(&runtime);

    let action = wrap_action(&handle, action_data).expect("wrap_action");
    // No outer scope_context here — `wrap_action` installs its own
    // via `RuntimeAccess::with_ctx`.
    action.invoke();
    action.invoke();
    drop(action);

    assert_eq!(RECORDED.lock().unwrap().clone(), vec![42, 42]);
    // Runtime is still alive; the dropped action's CCW unrooted the
    // script handle via the captured runtime handle.
    runtime.with_ctx_mut(|ctx| {
        // Root index 0 was used by wrap_action; after Drop it should
        // be `None`.
        assert!(ctx.external_root(0).is_none(), "drop should unroot");
    });
}

/// (2) Dropping the ComRc *outside* any explicit scope_context with
/// the runtime still alive cleanly unroots the script handle.
#[test]
fn drop_outside_scope_clears_external_root() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    RECORDED.lock().unwrap().clear();
    let (runtime, action_data) = build_runtime_with_recorder();
    let handle = RuntimeHandle::from_rc(&runtime);
    let action = wrap_action(&handle, action_data).expect("wrap_action");

    // Confirm the root is currently occupied.
    runtime.with_ctx_mut(|ctx| {
        assert!(ctx.external_root(0).is_some(), "wrap_action must have rooted");
    });

    // Drop with no surrounding scope_context. The CCW's `Drop` must
    // still tear down the root via the captured RuntimeHandle.
    drop(action);

    runtime.with_ctx_mut(|ctx| {
        assert!(
            ctx.external_root(0).is_none(),
            "drop must unroot via RuntimeHandle"
        );
    });
}

/// (3) Dropping the ComRc *after* the runtime has been dropped must
/// complete without panic, leak, or UAF. The Weak upgrade returns
/// `None` and the CCW's `Drop` is a quiet no-op.
#[test]
fn drop_after_runtime_drop_is_safe() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    RECORDED.lock().unwrap().clear();
    let (runtime, action_data) = build_runtime_with_recorder();
    let handle = RuntimeHandle::from_rc(&runtime);
    let action = wrap_action(&handle, action_data).expect("wrap_action");

    drop(runtime);
    assert!(handle.is_dangling(), "Weak upgrade should fail after runtime drop");

    // This must not panic / abort / segfault.
    drop(action);
}

/// (4) Invoking the ComRc after the runtime has been dropped must
/// gracefully no-op (the thunk's `try_with_ctx` returns `None`).
#[test]
fn invoke_after_runtime_drop_is_silent_noop() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    RECORDED.lock().unwrap().clear();
    let (runtime, action_data) = build_runtime_with_recorder();
    let handle = RuntimeHandle::from_rc(&runtime);
    let action = wrap_action(&handle, action_data).expect("wrap_action");

    drop(runtime);
    assert!(handle.is_dangling(), "Weak upgrade should fail after runtime drop");

    // Must not panic; must not append to RECORDED (the thunk early-
    // returns when `try_with_ctx` yields None).
    action.invoke();
    action.invoke();

    assert!(
        RECORDED.lock().unwrap().is_empty(),
        "invoke after runtime drop must not record"
    );

    // And dropping the action itself afterward must still be safe.
    drop(action);
}

/// (5) `wrap_action` called with a dangling handle errors loudly
/// rather than silently leaking a root.
#[test]
fn dangling_handle_errors_at_wrap_time() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let handle = RuntimeHandle::dangling();
    let dummy = Data::BoxRef {
        idx: 0,
        generation: 0,
    };
    let err = match wrap_action(&handle, dummy) {
        Ok(_) => panic!("dangling handle should error"),
        Err(e) => e,
    };
    assert!(
        format!("{}", err).contains("dangling RuntimeHandle"),
        "unexpected error: {err}"
    );
}
