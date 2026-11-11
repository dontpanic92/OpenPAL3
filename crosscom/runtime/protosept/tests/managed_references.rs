use p7::interpreter::context::{Context, Data};

#[test]
fn retained_receiver_can_be_captured_by_nested_host_callbacks() {
    let source = r#"
        @foreign(dispatcher="host.invoke", type_tag="Host")
        proto Host { fn with_body(refmut self, body: box<Action>); }
        @foreign(dispatcher="action.invoke", type_tag="Action")
        proto Action { fn invoke(self: &Self); }
        struct[Host] Runner() {
            fn with_body(refmut self, body: box<Action>) { body.invoke(); }
        }
        struct Screen(pub count: int) {
            fn render(self: &Self, host: box<Host>) {
                host.with_body(() => {
                    host.with_body(() => { self.count += 1; });
                });
            }
        }
        fn entry() -> int {
            let screen = &Screen(0);
            screen.render(box(Runner()));
            screen.count
        }
    "#;
    let module = p7::compile(source.into()).expect("retained receivers can escape in callbacks");
    let mut context = Context::new();
    context.load_module(module);
    context.push_function("entry", vec![]);
    context.resume().unwrap();
    assert_eq!(context.stack[0].stack.pop(), Some(Data::Int(1)));
}
