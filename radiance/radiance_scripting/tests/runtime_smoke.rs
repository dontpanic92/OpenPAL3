use p7::interpreter::context::Data;
use radiance_scripting::ScriptHost;

#[test]
fn runtime_calls_loaded_functions_and_stores_state() {
    let runtime = ScriptHost::new();
    runtime
        .load_source(
            r#"
pub fn init() -> box<int> {
    box(7)
}

pub fn ping(state: box<int>, dt: float) -> int {
    *state + 1
}
"#,
        )
        .expect("load script");

    let state = runtime
        .call_returning_data("init", Vec::new())
        .expect("init");
    let handle = runtime.root(state.clone());
    assert_eq!(runtime.deref_handle(handle), Some(state.clone()));

    let result = runtime
        .call_returning_data("ping", vec![state, Data::Float(0.25)])
        .expect("ping");
    assert_eq!(result, Data::Int(8));
}

#[test]
fn stored_state_survives_gc_compaction() {
    let runtime = ScriptHost::new();
    runtime
        .load_source(
            r#"
pub fn warmup() {
    let garbage = box(1);
}

pub fn init() -> box<int> {
    box(7)
}

pub fn ping(state: box<int>) -> int {
    *state + 1
}
"#,
        )
        .expect("load script");

    runtime.call_void("warmup", Vec::new()).expect("warmup");
    let state = runtime
        .call_returning_data("init", Vec::new())
        .expect("init");
    let handle = runtime.root(state);

    let _ = runtime.with_ctx_mut(|ctx| ctx.collect_garbage());

    let state = runtime
        .deref_handle(handle)
        .expect("rooted state should survive GC");
    assert_eq!(
        runtime
            .call_returning_data("ping", vec![state])
            .expect("ping"),
        Data::Int(8)
    );
}

#[test]
fn runtime_calls_script_owned_director_methods() {
    let runtime = ScriptHost::new();
    runtime
        .load_source(
            r#"
import radiance;
import immediate_director;

pub struct[immediate_director.IImmediateDirector, radiance.IDirector] First(pub trigger: box<array<int>>) {
    pub fn activate(self: ref<Self>) -> int { 0 }
    pub fn deactivate(self: ref<Self>) -> int { 0 }
    pub fn render_im(self: ref<Self>, ui: box<immediate_director.IUiHost>, dt: float) -> int { 0 }
    pub fn update(self: ref<Self>, dt: float) -> ?box<radiance.IDirector> {
        if self.trigger[0] != 0 {
            self.trigger[0] = 0;
            return make_second();
        }
        return null;
    }
}

pub struct[immediate_director.IImmediateDirector, radiance.IDirector] Second() {
    pub fn activate(self: ref<Self>) -> int { 0 }
    pub fn deactivate(self: ref<Self>) -> int { 0 }
    pub fn render_im(self: ref<Self>, ui: box<immediate_director.IUiHost>, dt: float) -> int { 0 }
    pub fn update(self: ref<Self>, dt: float) -> ?box<radiance.IDirector> {
        return null;
    }
}

fn make_second() -> box<radiance.IDirector> {
    return box(Second()) as box<radiance.IDirector>;
}

pub fn init() -> box<immediate_director.IImmediateDirector> {
    let trigger: array<int> = [1];
    return box(First(box(trigger))) as box<immediate_director.IImmediateDirector>;
}
"#,
        )
        .expect("load script director");

    let director = runtime
        .call_returning_data("init", Vec::new())
        .expect("init director");
    let result = runtime
        .call_method_returning_data(director, "update", vec![Data::Float(0.016)])
        .expect("update director");

    // Phase 6: update returns `?box<radiance.IDirector>`
    // instead of the legacy 0-or-1 array shape. First's trigger is
    // initially 1, so the first update returns Some(next).
    match result {
        Data::Some(_) => {}
        Data::ProtoBoxRef { .. } | Data::BoxRef { .. } => {}
        other => panic!("expected Some/box return for transition, got {other:?}"),
    }
}
