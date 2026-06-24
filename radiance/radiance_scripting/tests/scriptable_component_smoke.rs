//! Phase 1 of the Pal4ActorController script migration: end-to-end
//! smoke that a script-side `struct[radiance.IComponent]` can be
//! reverse-wrapped via the auto-generated `wrap_component` helper,
//! attached to a fresh `IEntity`, and driven through the full
//! load / update / unload lifecycle by the engine container.
//!
//! Mirrors `wrap_director_smoke.rs` in shape. Asserts:
//!
//! 1. Direct CCW dispatch: calling `on_loading` / `on_updating(dt)` /
//!    `on_unloading` on a `ComRc<IComponent>` reaches the p7 struct.
//! 2. Container dispatch: when the same component is attached via
//!    `IEntity::add_component`, the engine's exactly-once lifecycle
//!    fires `on_loading` (on `load()`), `on_updating` (on `update(dt)`),
//!    and `on_unloading` (on `unload()`) — same call sequence as Rust
//!    components.
//! 3. Drop after host drop is safe (no proto re-entry).

use std::sync::Mutex;

use crosscom::{ComInterface, ComRc};
use p7::errors::RuntimeError;
use p7::interpreter::context::Context;
use radiance::comdef::{IComponent, IEntity};
use radiance::scene::CoreEntity;
use radiance_scripting::ScriptHost;
use radiance_scripting::script_bridges::radiance::wrap_component;

const SCRIPT: &str = r#"
import radiance;

@intrinsic(name="comp_test.record_event")
fn record_event(seed: int, event: int);

struct[radiance.IComponent] StubComp(
    seed: int,
) {
    pub fn on_loading(self: refmut<Self>) -> int {
        record_event(self.seed, 1);
        0
    }
    pub fn on_updating(self: refmut<Self>, dt: float) -> int {
        record_event(self.seed, 2);
        0
    }
    pub fn on_unloading(self: refmut<Self>) -> int {
        record_event(self.seed, 3);
        0
    }
}

pub fn make_stub(seed: int) -> box<radiance.IComponent> {
    let c = box(StubComp(seed));
    c as box<radiance.IComponent>
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

fn host_runtime_handle(host: &std::rc::Rc<ScriptHost>) -> crosscom_protosept::RuntimeHandle {
    let mut out: Option<crosscom_protosept::RuntimeHandle> = None;
    <ScriptHost as crosscom_protosept::RuntimeAccess>::with_ctx(host, &mut |_ctx| {
        let h = crosscom_protosept::with_services(|s| s.runtime_handle())
            .expect("with_services inside scope");
        out = Some(h);
    });
    out.expect("RuntimeAccess::with_ctx ran body")
}

fn make_wrapped_component(host: &std::rc::Rc<ScriptHost>, seed: i64) -> ComRc<IComponent> {
    let data = host
        .call_returning_data("make_stub", vec![p7::interpreter::context::Data::Int(seed)])
        .expect("make_stub");
    let handle = host_runtime_handle(host);
    wrap_component(&handle, data).expect("wrap_component")
}

#[test]
fn direct_dispatch_invokes_each_lifecycle_method() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EVENTS.lock().unwrap().clear();

    let host = ScriptHost::new();
    host.with_ctx_mut(|ctx| {
        ctx.register_host_function("comp_test.record_event".to_string(), record_event_host_fn);
    });
    host.load_source(SCRIPT).expect("load_source");

    let component = make_wrapped_component(&host, 42);

    component.on_loading();
    component.on_updating(0.016);
    component.on_updating(0.032);
    component.on_unloading();

    assert_eq!(
        *EVENTS.lock().unwrap(),
        vec![(42, 1), (42, 2), (42, 2), (42, 3)],
        "direct CCW dispatch should hit each script-side lifecycle hook"
    );

    drop(component);

    assert_eq!(
        *EVENTS.lock().unwrap(),
        vec![(42, 1), (42, 2), (42, 2), (42, 3)],
        "drop must not re-fire any hook (no implicit release hook)"
    );
}

#[test]
fn container_dispatch_drives_load_update_unload_exactly_once() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EVENTS.lock().unwrap().clear();

    let host = ScriptHost::new();
    host.with_ctx_mut(|ctx| {
        ctx.register_host_function("comp_test.record_event".to_string(), record_event_host_fn);
    });
    host.load_source(SCRIPT).expect("load_source");

    let component = make_wrapped_component(&host, 7);
    let entity: ComRc<IEntity> =
        ComRc::<IEntity>::from_object(CoreEntity::new("script_comp_host".to_string(), true));

    // Attach before load → on_loading fires on the entity's load().
    let comp_uuid = uuid::Uuid::from_bytes(IComponent::INTERFACE_ID);
    entity.add_component(comp_uuid, component.clone());

    // Lifecycle: load → update(dt) twice → unload.
    entity.load();
    entity.update(0.016);
    entity.update(0.032);
    entity.unload();

    assert_eq!(
        *EVENTS.lock().unwrap(),
        vec![(7, 1), (7, 2), (7, 2), (7, 3)],
        "engine container dispatch should fire each hook exactly once per call"
    );

    // Idempotency: a second load() after unload is a no-op because
    // `drain_loaded_for_unload` removes the component from the entity
    // bag. Confirm no spurious hooks fire.
    entity.load();
    entity.unload();

    assert_eq!(
        *EVENTS.lock().unwrap(),
        vec![(7, 1), (7, 2), (7, 2), (7, 3)],
        "load/unload after drain must not re-fire any hook"
    );

    drop(component);
    drop(entity);
}

#[test]
fn wrap_component_drop_after_host_drop_is_safe() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = EVENTS.lock().unwrap().len();

    let host = ScriptHost::new();
    host.with_ctx_mut(|ctx| {
        ctx.register_host_function("comp_test.record_event".to_string(), record_event_host_fn);
    });
    host.load_source(SCRIPT).expect("load_source");

    let component = make_wrapped_component(&host, 99);
    let handle = host_runtime_handle(&host);
    drop(host);
    assert!(handle.is_dangling(), "host gone => handle dangling");
    drop(component); // no panic, no recorded hooks

    let after = EVENTS.lock().unwrap().clone();
    assert!(
        !after[before..].iter().any(|(s, _)| *s == 99),
        "no lifecycle hook should record when runtime is gone; got {after:?}"
    );
}
