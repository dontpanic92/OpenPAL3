//! Phase 2 (B3) integration test: a `ComRc<IAction>` produced by the
//! `com.invoke` dispatcher (via a script-impl SAM-coerced callback)
//! survives across multiple `ScriptHost::call_*` invocations and
//! remains invokable outside any active `scope_context`. Exercises
//! the `RuntimeHandle` decoupling on the *real* `ScriptHost` (not
//! the `TestRuntime` used in `crosscom-protosept`'s unit-level
//! lifetime tests).

use std::cell::RefCell;
use std::sync::Mutex;

use crosscom::{ComRc, IAction};
use crosscom_protosept::{with_services, wrap_proto};
use p7::errors::RuntimeError;
use p7::interpreter::context::Context;
use radiance_scripting::ScriptHost;

const SOURCE: &str = r#"
import crosscom;

@intrinsic(name="test.capture_action")
fn capture_action(body: box<crosscom.IAction>);

@intrinsic(name="test.record_invocation")
fn record_invocation();

struct[crosscom.IAction] Callback() {
    pub fn invoke(self: ref<Self>) -> int {
        record_invocation();
        0
    }
}

pub fn save_callback() {
    let cb = box(Callback());
    capture_action(cb as box<crosscom.IAction>);
}

pub fn run_unrelated() {
    // No-op script call between save and invoke; exists so we have a
    // second `ScriptHost::call_void` activation between the
    // `wrap_proto` call and the post-call `invoke` to demonstrate
    // that the captured ComRc survives across distinct
    // `scope_context` activations.
}
"#;

// Thread-local because tests in cargo's harness run in parallel; we
// want each test invocation's captures to be isolated. But each test
// runs in its own thread, so a thread-local naturally segregates.
thread_local! {
    static CAPTURED: RefCell<Vec<ComRc<IAction>>> = const { RefCell::new(Vec::new()) };
    static INVOCATIONS: RefCell<i64> = const { RefCell::new(0) };
}

// Global lock so we only ever drive one ScriptHost-driven test at a
// time across the file (defensive — the thread_locals already
// isolate state, but keeps the file's surface tidy).
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn capture_action_host_fn(ctx: &mut Context) -> Result<(), RuntimeError> {
    let frame = ctx.stack_frame_mut()?;
    let arg = frame
        .stack
        .pop()
        .ok_or_else(|| RuntimeError::Other("capture_action: missing arg".into()))?;
    let handle = with_services(|s| s.runtime_handle())
        .map_err(|e| RuntimeError::Other(format!("capture_action: services: {}", e)))?;
    let com = wrap_proto::<IAction>(&handle, arg)
        .map_err(|e| RuntimeError::Other(format!("capture_action: wrap_proto: {}", e)))?;
    CAPTURED.with(|c| c.borrow_mut().push(com));
    Ok(())
}

fn record_invocation_host_fn(_ctx: &mut Context) -> Result<(), RuntimeError> {
    INVOCATIONS.with(|c| *c.borrow_mut() += 1);
    Ok(())
}

#[test]
fn captured_action_survives_across_script_host_calls() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    CAPTURED.with(|c| c.borrow_mut().clear());
    INVOCATIONS.with(|c| *c.borrow_mut() = 0);

    let host = ScriptHost::new();
    host.with_ctx_mut(|ctx| {
        ctx.register_host_function("test.capture_action".to_string(), capture_action_host_fn);
        ctx.register_host_function(
            "test.record_invocation".to_string(),
            record_invocation_host_fn,
        );
    });
    host.load_source(SOURCE).expect("load_source");

    // First scope_context activation: the script constructs a
    // Callback box, passes it across the dispatcher's `com.invoke`
    // path (into our host fn), which builds a ComRc<IAction> via
    // `wrap_proto` and stores it.
    host.call_void("save_callback", vec![])
        .expect("save_callback");

    let captured = CAPTURED.with(|c| c.borrow().last().cloned().expect("captured action"));
    assert_eq!(INVOCATIONS.with(|c| *c.borrow()), 0);

    // Invoke the captured ComRc with NO scope_context active. The
    // CCW must re-enter the runtime via the RuntimeHandle.
    captured.invoke();
    assert_eq!(INVOCATIONS.with(|c| *c.borrow()), 1);

    // Second scope_context activation (a completely unrelated
    // script call) — the captured ComRc's outer scope from before
    // is well and truly dead.
    host.call_void("run_unrelated", vec![])
        .expect("run_unrelated");
    assert_eq!(INVOCATIONS.with(|c| *c.borrow()), 1);

    // Invoke again after the second activation. Still works.
    captured.invoke();
    captured.invoke();
    assert_eq!(INVOCATIONS.with(|c| *c.borrow()), 3);

    // Drop the captured ComRc; its CCW's Drop must unroot via the
    // RuntimeHandle. This currently runs *outside* any active
    // scope_context; under Phase B3 that's expected to be fine.
    drop(captured);
    CAPTURED.with(|c| c.borrow_mut().clear());

    // ScriptHost still alive and functional after drop.
    host.call_void("run_unrelated", vec![])
        .expect("run_unrelated again");
}

#[test]
fn dropping_script_host_with_outstanding_action_is_safe() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    CAPTURED.with(|c| c.borrow_mut().clear());
    INVOCATIONS.with(|c| *c.borrow_mut() = 0);

    let host = ScriptHost::new();
    host.with_ctx_mut(|ctx| {
        ctx.register_host_function("test.capture_action".to_string(), capture_action_host_fn);
        ctx.register_host_function(
            "test.record_invocation".to_string(),
            record_invocation_host_fn,
        );
    });
    host.load_source(SOURCE).expect("load_source");

    host.call_void("save_callback", vec![])
        .expect("save_callback");
    let captured = CAPTURED.with(|c| c.borrow().last().cloned().expect("captured"));

    // Drop the host (and all its strong refs) before the captured
    // ComRc. The CCW must hold only a Weak; dropping it after the
    // runtime is gone must not panic and must not record an
    // invocation.
    drop(host);
    CAPTURED.with(|c| c.borrow_mut().clear()); // release the static's strong ref to `captured`'s sibling
    // `captured` is the only remaining strong ref; the inner Weak
    // should fail to upgrade.

    // Invoking on a dead runtime: thunk no-ops.
    captured.invoke();
    assert_eq!(INVOCATIONS.with(|c| *c.borrow()), 0);

    // Final drop must be safe.
    drop(captured);
}
