//! Phase 4 (B2 + B4): integration test for the `wrap_director`
//! convenience helper. Confirms that a `ComRc<IDirector>` obtained
//! via `wrap_director` invokes activate/update on the script side
//! and fires the script's `deactivate()` (via the proto_ccw release
//! hook) when the ComRc is dropped.
//!
//! Per the Phase 4 release-hook constraint
//! ([`ProtoSpec::release_method`](`crosscom_protosept::ProtoSpec`)),
//! the script's struct must conform to a proto declaring
//! `deactivate`. The Rust-side engine interface `radiance.IDirector`
//! does not declare it, so this test introduces an auxiliary
//! script-only proto `IDirectorLifecycle` with `deactivate`, and the
//! script struct conforms to both `radiance.IDirector` (for vtable
//! shape) and `IDirectorLifecycle` (for the deactivate hook).

use std::sync::Mutex;

use crosscom::ComRc;
use p7::errors::RuntimeError;
use p7::interpreter::context::Context;
use radiance::comdef::IDirector;
use radiance_scripting::{wrap_director, ScriptHost};

const SCRIPT: &str = r#"
import radiance;

@intrinsic(name="wd_test.record_event")
fn record_event(seed: int, event: int);

pub proto IDirectorLifecycle {
    fn deactivate(self: ref<IDirectorLifecycle>);
}

struct[radiance.IDirector, IDirectorLifecycle] StubDir(
    seed: int,
) {
    pub fn activate(self: ref<Self>) -> int {
        record_event(self.seed, 1);
        0
    }
    pub fn update(self: ref<Self>, dt: float) -> ?box<radiance.IDirector> {
        record_event(self.seed, 2);
        return null;
    }
    pub fn deactivate(self: ref<Self>) {
        record_event(self.seed, 3);
    }
}

pub fn make_stub(seed: int) -> box<radiance.IDirector> {
    let d = box(StubDir(seed));
    d as box<radiance.IDirector>
}
"#;

static EVENTS: Mutex<Vec<(i64, i64)>> = Mutex::new(Vec::new());
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn record_event_host_fn(ctx: &mut Context) -> Result<(), RuntimeError> {
    let frame = ctx.stack_frame_mut()?;
    let event = frame.stack.pop().expect("event arg");
    let seed = frame.stack.pop().expect("seed arg");
    match (seed, event) {
        (p7::interpreter::context::Data::Int(s), p7::interpreter::context::Data::Int(e)) => {
            EVENTS.lock().unwrap().push((s, e));
            Ok(())
        }
        other => panic!("expected (int, int) record_event args, got {:?}", other),
    }
}

fn host_runtime_handle(
    host: &std::rc::Rc<ScriptHost>,
) -> crosscom_protosept::RuntimeHandle {
    // Re-enter the runtime via the same path the dispatcher uses to
    // pull a RuntimeHandle out of the active services bundle.
    let mut out: Option<crosscom_protosept::RuntimeHandle> = None;
    <ScriptHost as crosscom_protosept::RuntimeAccess>::with_ctx(host, &mut |_ctx| {
        let h = crosscom_protosept::with_services(|s| s.runtime_handle())
            .expect("with_services inside scope");
        out = Some(h);
    });
    out.expect("RuntimeAccess::with_ctx ran body")
}

#[test]
fn wrap_director_runs_activate_update_and_fires_deactivate_on_drop() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EVENTS.lock().unwrap().clear();

    let host = ScriptHost::new();
    host.with_ctx_mut(|ctx| {
        ctx.register_host_function("wd_test.record_event".to_string(), record_event_host_fn);
    });
    host.load_source(SCRIPT).expect("load_source");

    let dir_data = host
        .call_returning_data("make_stub", vec![p7::interpreter::context::Data::Int(42)])
        .expect("make_stub");

    let handle = host_runtime_handle(&host);
    let director: ComRc<IDirector> = wrap_director(&handle, dir_data).expect("wrap_director");

    director.activate(); // (42, 1)
    let next = director.update(0.016); // (42, 2)
    assert!(next.is_none(), "Null update -> None");

    assert_eq!(
        *EVENTS.lock().unwrap(),
        vec![(42, 1), (42, 2)],
        "activate then update recorded before drop"
    );

    // Dropping the ComRc<IDirector> fires the release hook, which
    // dispatches `deactivate()` through the auxiliary
    // IDirectorLifecycle proto the script struct conforms to.
    drop(director);

    assert_eq!(
        *EVENTS.lock().unwrap(),
        vec![(42, 1), (42, 2), (42, 3)],
        "deactivate fired on final ComRc drop via release hook"
    );
}

#[test]
fn wrap_director_drop_after_host_drop_is_safe() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = EVENTS.lock().unwrap().len();

    let host = ScriptHost::new();
    host.with_ctx_mut(|ctx| {
        ctx.register_host_function("wd_test.record_event".to_string(), record_event_host_fn);
    });
    host.load_source(SCRIPT).expect("load_source");

    let dir_data = host
        .call_returning_data("make_stub", vec![p7::interpreter::context::Data::Int(99)])
        .expect("make_stub");

    let handle = host_runtime_handle(&host);
    let director: ComRc<IDirector> = wrap_director(&handle, dir_data).expect("wrap_director");

    drop(host);
    assert!(handle.is_dangling(), "host gone => handle dangling");
    drop(director); // no panic, no recorded deactivate

    let after = EVENTS.lock().unwrap().clone();
    assert!(
        !after[before..].iter().any(|(s, _)| *s == 99),
        "no deactivate should record when runtime is gone; got {after:?}"
    );
}
