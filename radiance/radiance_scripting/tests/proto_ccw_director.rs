//! Phase 3 (B1b) integration test: end-to-end runtime-typed CCW for
//! the real `radiance.IDirector` under the real `ScriptHost`.
//!
//! Registers `radiance.IDirector` with the runtime-typed CCW factory,
//! loads a script declaring a `struct[radiance.IDirector] StubDir`,
//! constructs the script side, wraps the resulting box as a Rust-side
//! `ComRc<IDirector>`, then drives `activate()` and `update(dt)`
//! from Rust. Asserts the script methods ran and the
//! `OptionalForeign` return marshals correctly.
//!
//! Pre-validates Phase 4's `wrap_director` — the only delta will be
//! a convenience helper that bakes the `ProtoSpec` for
//! `radiance.IDirector` and exposes a typed
//! `wrap_director(handle, data)`.

use std::sync::Mutex;

use crosscom::{ComInterface, ComRc};
use crosscom_protosept::{
    register_proto_ccw, with_services, wrap_proto, ArgKind, MethodSpec, ProtoSpec, RetKind,
};
use p7::errors::RuntimeError;
use p7::interpreter::context::Context;
use radiance::comdef::IDirector;
use radiance_scripting::ScriptHost;

const SCRIPT: &str = r#"
import radiance;

@intrinsic(name="test.record_director_event")
fn record_event(seed: int, event: int);

struct[radiance.IDirector] StubDir(
    seed: int,
    return_self: int,
) {
    pub fn activate(self: ref<Self>) -> int {
        record_event(self.seed, 1);
        0
    }
    pub fn update(self: ref<Self>, dt: float) -> ?box<radiance.IDirector> {
        record_event(self.seed, 2);
        if self.return_self != 0 {
            let next = box(StubDir(self.seed + 100, 0));
            return next as box<radiance.IDirector>;
        }
        return null;
    }
    pub fn deactivate(self: ref<Self>) -> int {
        // Phase 4 release hook fires this on final ComRc drop.
        record_event(self.seed, 3);
        0
    }
}

pub fn make_stub(seed: int, return_self: int) -> box<radiance.IDirector> {
    let d = box(StubDir(seed, return_self));
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

fn ensure_director_registered() {
    static REGISTERED: Mutex<bool> = Mutex::new(false);
    let mut g = REGISTERED.lock().unwrap();
    if *g {
        return;
    }
    register_proto_ccw(ProtoSpec {
        uuid: IDirector::INTERFACE_ID,
        type_tag: "radiance.comdef.IDirector".into(),
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
                    type_tag: "radiance.comdef.IDirector".into(),
                    uuid: IDirector::INTERFACE_ID,
                },
            },
        ],
        release_method: Some("deactivate".into()),
        additional_query_uuids: vec![],
    })
    .expect("register radiance.IDirector");
    *g = true;
}

#[test]
fn idirector_activate_and_null_update_round_trip() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EVENTS.lock().unwrap().clear();
    ensure_director_registered();

    let host = ScriptHost::new();
    host.with_ctx_mut(|ctx| {
        ctx.register_host_function("test.record_director_event".to_string(), record_event_host_fn);
    });
    host.load_source(SCRIPT).expect("load_source");

    let dir_data = host
        .call_returning_data(
            "make_stub",
            vec![
                p7::interpreter::context::Data::Int(11),
                p7::interpreter::context::Data::Int(0),
            ],
        )
        .expect("make_stub");

    // wrap_proto runs inside a host method? No — we're outside any
    // ScriptHost::call here. The Phase 2 RuntimeHandle path is what
    // makes that safe.
    let runtime_handle = host.with_inner_for_test(|services| services.runtime_handle.clone());

    let director: ComRc<IDirector> = wrap_proto(&runtime_handle, dir_data).expect("wrap_proto");

    director.activate();
    let next = director.update(0.016);
    assert!(next.is_none(), "Null update returns None");

    assert_eq!(
        *EVENTS.lock().unwrap(),
        vec![(11, 1), (11, 2)],
        "activate then update recorded"
    );
}

#[test]
fn idirector_update_returning_some_recursively_wraps() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EVENTS.lock().unwrap().clear();
    ensure_director_registered();

    let host = ScriptHost::new();
    host.with_ctx_mut(|ctx| {
        ctx.register_host_function("test.record_director_event".to_string(), record_event_host_fn);
    });
    host.load_source(SCRIPT).expect("load_source");

    let dir_data = host
        .call_returning_data(
            "make_stub",
            vec![
                p7::interpreter::context::Data::Int(5),
                p7::interpreter::context::Data::Int(1),
            ],
        )
        .expect("make_stub");

    let runtime_handle = host.with_inner_for_test(|services| services.runtime_handle.clone());
    let director: ComRc<IDirector> = wrap_proto(&runtime_handle, dir_data).expect("wrap_proto");

    director.activate(); // (5,1)
    let next = director.update(0.016); // (5,2)
    let next = next.expect("Some next director");
    next.activate(); // (105,1)
    let after = next.update(0.016); // (105,2)
    assert!(after.is_none(), "second-level StubDir has return_self=0");

    assert_eq!(
        *EVENTS.lock().unwrap(),
        vec![(5, 1), (5, 2), (105, 1), (105, 2)],
    );

    drop(next);
    drop(director);
    // ScriptHost still alive and functional.
    drop(host);
}

/// Helper trait-extension shim: ScriptHost doesn't expose its inner
/// services bundle publicly. For tests we need access to the
/// `RuntimeHandle` it installed. Add a test-only method via a local
/// extension trait wired through the public `with_ctx_mut` to reach
/// services indirectly — by going through `with_services` after
/// entering a scope.
trait ScriptHostTestExt {
    fn with_inner_for_test<R>(
        &self,
        body: impl FnOnce(&mut radiance_scripting::RuntimeServices) -> R,
    ) -> R;
}

impl ScriptHostTestExt for std::rc::Rc<ScriptHost> {
    fn with_inner_for_test<R>(
        &self,
        body: impl FnOnce(&mut radiance_scripting::RuntimeServices) -> R,
    ) -> R {
        // Re-enter the runtime via the same path the dispatcher uses:
        // call into the script's runtime, then inside the
        // installed `scope`, read services via `with_services`.
        let mut out: Option<R> = None;
        let mut body_slot = Some(body);
        self.with_ctx_mut(|_ctx| {
            // We're inside `with_inner` here (with_ctx_mut delegates),
            // but `scope` is not installed at this depth. Install it
            // ourselves so `with_services` works.
            //
            // This is exactly what `ScriptHost::call_inner` does for
            // a script call. We piggyback on `RuntimeAccess::with_ctx`
            // which also installs `scope` + `scope_context`.
        });
        // Simpler path: use the RuntimeAccess impl to do the work for
        // us. It installs `scope` + `scope_context`, and inside the
        // closure we can call `with_services`.
        <ScriptHost as crosscom_protosept::RuntimeAccess>::with_ctx(self, &mut |_ctx| {
            let b = body_slot.take().expect("body consumed twice");
            let r = with_services(|s| {
                // Downcast HostServices to RuntimeServices.
                let any_s: &mut dyn std::any::Any = s as _;
                let rs = any_s
                    .downcast_mut::<radiance_scripting::RuntimeServices>()
                    .expect("services are RuntimeServices");
                b(rs)
            })
            .expect("with_services inside scope");
            out = Some(r);
        });
        out.expect("RuntimeAccess::with_ctx ran body")
    }
}
