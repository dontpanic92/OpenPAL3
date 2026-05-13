const RECURSIVE_SRC: &str = r#"
pub struct Node(pub kind: int, pub children: array<Node>);
pub fn make() -> Node {
    let children: array<Node> = [];
    Node(1, [Node(2, children)])
}
"#;

#[test]
fn recursive_struct_compiles_via_runtime() {
    let runtime = radiance_scripting::ScriptHost::new();
    runtime
        .load_source(RECURSIVE_SRC)
        .expect("recursive struct compiles");
}
