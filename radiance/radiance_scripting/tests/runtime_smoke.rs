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
import ui_host;

pub struct[director.ImmediateDirector] First(pub trigger: box<array<int>>) {
    pub fn activate(self: ref<Self>) {}
    pub fn deactivate(self: ref<Self>) {}
    pub fn render_im(self: ref<Self>, ui: box<ui_host.IUiHost>, dt: float) {}
    pub fn update(self: ref<Self>, dt: float) -> array<box<director.ImmediateDirector>> {
        if self.trigger[0] != 0 {
            self.trigger[0] = 0;
            return [make_second()];
        }
        let result: array<box<director.ImmediateDirector>> = [];
        return result;
    }
}

pub struct[director.ImmediateDirector] Second() {
    pub fn activate(self: ref<Self>) {}
    pub fn deactivate(self: ref<Self>) {}
    pub fn render_im(self: ref<Self>, ui: box<ui_host.IUiHost>, dt: float) {}
    pub fn update(self: ref<Self>, dt: float) -> array<box<director.ImmediateDirector>> {
        let result: array<box<director.ImmediateDirector>> = [];
        return result;
    }
}

fn make_second() -> box<director.ImmediateDirector> {
    return box(Second()) as box<director.ImmediateDirector>;
}

pub fn init() -> box<director.ImmediateDirector> {
    let trigger: array<int> = [1];
    return box(First(box(trigger))) as box<director.ImmediateDirector>;
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

    match result {
        Data::Array(values) => assert_eq!(values.len(), 1),
        other => panic!("expected one returned director, got {other:?}"),
    }
}
