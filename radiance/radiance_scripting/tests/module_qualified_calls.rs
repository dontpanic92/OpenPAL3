use p7::interpreter::context::Data;
use radiance_scripting::ScriptHost;

const APP: &str = r#"
import left;
import right;

pub fn root_value() -> int {
    return 1;
}
"#;

const LEFT: &str = r#"
pub fn init(v: int) -> int {
    return v + 10;
}
"#;

const RIGHT: &str = r#"
pub fn init(v: int) -> int {
    return v + 20;
}
"#;

#[test]
fn dispatches_duplicate_function_names_by_module() {
    let host = ScriptHost::new();
    host.add_binding("left", LEFT);
    host.add_binding("right", RIGHT);
    host.load_source(APP).expect("app root should compile");

    assert!(host.has_function("root_value"));
    assert!(host.has_module_function("left", "init"));
    assert!(host.has_module_function("right", "init"));

    let left = host
        .call_module_returning_data("left", "init", vec![Data::Int(5)])
        .expect("left init should dispatch");
    let right = host
        .call_module_returning_data("right", "init", vec![Data::Int(5)])
        .expect("right init should dispatch");

    assert_eq!(left, Data::Int(15));
    assert_eq!(right, Data::Int(25));
}
