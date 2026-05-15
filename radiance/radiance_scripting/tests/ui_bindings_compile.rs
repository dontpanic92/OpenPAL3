use p7::interpreter::context::{Context, Data};

#[test]
fn ui_bindings_compile() {
    p7::compile_with_provider(
        radiance_scripting::ui_walker::UI_BINDINGS_P7.to_string(),
        Box::new(p7::NoModuleProvider),
    )
    .expect("ui bindings compile");
}

#[test]
fn constructor_returns_uinode_struct_shape() {
    let source = format!(
        "{}\r\npub fn make() -> UiNode {{\r\n    window(\"t\", 100.0, 200.0, 0, [text(\"hi\")])\r\n}}\r\n",
        radiance_scripting::ui_walker::UI_BINDINGS_P7
    );
    let module = p7::compile_with_provider(source, Box::new(p7::NoModuleProvider))
        .expect("ui bindings with constructor call compile");

    let mut ctx = Context::new();
    ctx.load_module(module);
    ctx.push_function("make", Vec::new());
    ctx.resume().expect("run make");

    let result = ctx.stack[0].stack.pop().expect("make returned a value");
    let owned = radiance_scripting::ui_walker::owned::resolve(&ctx, &result)
        .expect("resolve UiNode to owned tree");
    assert_eq!(owned.kind, 1);
    assert_eq!(owned.label, "t");
    assert_eq!(owned.children.len(), 1);
    assert_eq!(owned.children[0].kind, 5);
    assert_eq!(owned.children[0].label, "hi");

    let parent_ref = match result {
        Data::StructRef(idx) => idx,
        other => panic!("expected UiNode StructRef, got {other:?}"),
    };
    let fields = ctx.heap[parent_ref as usize].fields.clone();
    assert_eq!(fields.len(), 7);
    assert_eq!(fields[0], Data::Int(1));
    assert_eq!(fields[1], Data::String("t".into()));
    assert_eq!(fields[2], Data::Float(100.0));
    assert_eq!(fields[3], Data::Float(200.0));
    assert_eq!(fields[4], Data::Int(0));
    assert_eq!(fields[5], Data::Int(0));

    let children = match &fields[6] {
        Data::BoxRef { idx, generation } => match ctx.box_heap.get(*idx, *generation) {
            Ok(Data::Array(children)) => children.clone(),
            Ok(other) => panic!("expected boxed children array, got {other:?}"),
            Err(err) => panic!("expected boxed children array, got error {err:?}"),
        },
        other => panic!("expected boxed children array, got {other:?}"),
    };
    assert_eq!(children.len(), 1);

    let child_ref = match children[0] {
        Data::StructRef(idx) => idx,
        ref other => panic!("expected child UiNode StructRef, got {other:?}"),
    };
    let child_fields = &ctx.heap[child_ref as usize].fields;
    assert_eq!(child_fields.len(), 7);
    assert_eq!(child_fields[0], Data::Int(5));
    assert_eq!(child_fields[1], Data::String("hi".into()));
    assert_eq!(child_fields[2], Data::Float(0.0));
    assert_eq!(child_fields[3], Data::Float(0.0));
    assert_eq!(child_fields[4], Data::Int(0));
    assert_eq!(child_fields[5], Data::Int(0));
    match child_fields[6] {
        Data::BoxRef { idx, generation } => assert_eq!(
            ctx.box_heap.get(idx, generation).expect("box deref"),
            &Data::Array(std::rc::Rc::new(Vec::new()))
        ),
        ref other => panic!("expected boxed child children array, got {other:?}"),
    }
}

#[test]
fn imported_ui_module_constructors_return_walkable_nodes() {
    let mut provider = p7::InMemoryModuleProvider::new();
    provider.add_module(
        "ui".to_string(),
        radiance_scripting::ui_walker::UI_BINDINGS_P7.to_string(),
    );

    let source = r#"
import ui;

pub fn make() -> ui.UiNode {
    return ui.window("t", 100.0, 200.0, 0, [ui.text("hi")]);
}
"#;
    let module = p7::compile_with_provider(source.to_string(), Box::new(provider))
        .expect("script using import ui compiles");

    let mut ctx = Context::new();
    ctx.load_module(module);
    ctx.push_function("make", Vec::new());
    ctx.resume().expect("run make");

    let result = ctx.stack[0].stack.pop().expect("make returned a value");
    let owned = radiance_scripting::ui_walker::owned::resolve(&ctx, &result)
        .expect("resolve imported UiNode to owned tree");
    assert_eq!(owned.kind, 1);
    assert_eq!(owned.label, "t");
    assert_eq!(owned.children.len(), 1);
    assert_eq!(owned.children[0].kind, 5);
    assert_eq!(owned.children[0].label, "hi");
}
