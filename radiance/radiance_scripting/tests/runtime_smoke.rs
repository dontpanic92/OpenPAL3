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
import director;
import ui;

pub struct[director.Director] First() {
    pub fn activate(self: ref<Self>) {}
    pub fn deactivate(self: ref<Self>) {}
    pub fn render(self: ref<Self>, dt: float) -> ui.UiNode {
        return ui.text("first");
    }
    pub fn dispatch(self: ref<Self>, command_id: int) -> array<box<director.Director>> {
        if command_id == 7 {
            return [make_second()];
        }
        let result: array<box<director.Director>> = [];
        return result;
    }
    pub fn update(self: ref<Self>, dt: float) -> array<box<director.Director>> {
        let result: array<box<director.Director>> = [];
        return result;
    }
}

pub struct[director.Director] Second() {
    pub fn activate(self: ref<Self>) {}
    pub fn deactivate(self: ref<Self>) {}
    pub fn render(self: ref<Self>, dt: float) -> ui.UiNode {
        return ui.text("second");
    }
    pub fn dispatch(self: ref<Self>, command_id: int) -> array<box<director.Director>> {
        let result: array<box<director.Director>> = [];
        return result;
    }
    pub fn update(self: ref<Self>, dt: float) -> array<box<director.Director>> {
        let result: array<box<director.Director>> = [];
        return result;
    }
}

fn make_second() -> box<director.Director> {
    return box(Second()) as box<director.Director>;
}

pub fn init() -> box<director.Director> {
    return box(First()) as box<director.Director>;
}
"#,
        )
        .expect("load script director");

    let director = runtime
        .call_returning_data("init", Vec::new())
        .expect("init director");
    let result = runtime
        .call_method_returning_data(director, "dispatch", vec![Data::Int(7)])
        .expect("dispatch director command");

    match result {
        Data::Array(values) => assert_eq!(values.len(), 1),
        other => panic!("expected one returned director, got {other:?}"),
    }
}
