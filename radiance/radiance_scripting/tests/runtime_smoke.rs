use p7::interpreter::context::Data;
use radiance_scripting::ScriptRuntime;

#[test]
fn runtime_calls_loaded_functions_and_stores_state() {
    let mut runtime = ScriptRuntime::new();
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
    runtime.store_state(state.clone());
    assert_eq!(runtime.state_clone(), Some(state.clone()));

    let result = runtime
        .call_returning_data("ping", vec![state, Data::Float(0.25)])
        .expect("ping");
    assert_eq!(result, Data::Int(8));
}
