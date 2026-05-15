//! End-to-end test for the reverse-direction adapter
//! `script_proxy::wrap_action`: a Rust caller obtains a
//! `ComRc<IAction>` backed by a script struct, invokes it, and the
//! script method runs.

use crosscom_protosept::{scope_context, wrap_action};
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

use std::sync::Mutex;

static RECORDED: Mutex<Vec<i64>> = Mutex::new(Vec::new());

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

#[test]
fn host_can_invoke_script_struct_via_comrc_action() {
    {
        // Isolate from prior test runs in the same binary.
        RECORDED.lock().unwrap().clear();
    }

    let module = p7::compile(SOURCE.to_string()).expect("compile");
    let mut ctx = Context::new();
    ctx.register_host_function("recorder.set".to_string(), recorder_set_host_fn);
    ctx.load_module(module);

    // Drive the script to construct a `box<IAction>` backed by a
    // `Recorder` struct and pop it off the result stack.
    ctx.push_function("make_recorder", vec![Data::Int(7)]);
    ctx.resume().expect("make_recorder ran");
    let action_data = ctx.stack[0].stack.pop().expect("returned box");

    // Wrap as a ComRc<IAction> + invoke twice. Both invocations should
    // re-enter the interpreter and run `Recorder.invoke`, which calls
    // `record_invocation(self.seed)`.
    scope_context(&mut ctx, || {
        let action = wrap_action(unsafe { current_ctx() }, action_data).expect("wrap_action");
        action.invoke();
        action.invoke();
        // Drop here releases the ComRc → frees the CCW → unroots via
        // Drop on ScriptActionProxy.
        drop(action);
    });

    let recorded = RECORDED.lock().unwrap().clone();
    assert_eq!(recorded, vec![7, 7]);
}

/// Re-fetch the context pointer set by `scope_context` for use inside
/// the body. Since the closure passed to `scope_context` doesn't
/// receive `&mut Context`, we need this helper for the test.
///
/// Safety: only valid while `scope_context` is on the stack.
unsafe fn current_ctx<'a>() -> &'a mut Context {
    crosscom_protosept::with_context(|ctx| ctx as *mut Context)
        .expect("context not in scope")
        .as_mut()
        .unwrap()
}
