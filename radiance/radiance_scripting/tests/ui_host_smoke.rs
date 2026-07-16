//! H2 smoke test (per generated/plan.md): end-to-end SAM + crosscom +
//! p7 plumbing through an `IUiHost` host service.
//!
//! Each test loads a tiny p7 module that declares a script struct with
//! a `render(ui: box<IUiHost>, dt: float)` style entry point. The
//! Rust test wires a `RecordingUiHost` ComObject, hands the script a
//! `box<IUiHost>` foreign value, and verifies that:
//!
//! 1. The script's calls to `ui.text(...)`, `ui.button(...)`, etc.
//!    cross the host boundary and arrive in `RecordingUiHost` in the
//!    declared order.
//! 2. Closure bodies passed to pairing widgets (`ui.window_centered(
//!    ..., () => { ... })`) are SAM-coerced (§L2) into anonymous
//!    `struct[IAction]` impls and the host invokes them via
//!    `IAction.invoke()` — *between* the recorded `BodyEnter` /
//!    `BodyExit` markers.
//! 3. Captures of script-side `let` bindings are read correctly
//!    inside the closure body (proves the §L2 `self.field` rewrite
//!    + foreign-box passing).
//! 4. `button` returns `bool` (encoded as `int 0/1` across the C ABI)
//!    so scripts can branch on click.

use radiance_scripting::ScriptHost;
use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};

const NO_CAPTURE_SOURCE: &str = r#"
import radiance;

pub fn entry(ui: box<radiance.IUiHost>) -> int {
    ui.window_centered("hello", 100.0, 50.0, () => {
        ui.text("inside");
    });
    ui.text("after");
    0
}
"#;

const WITH_CAPTURE_SOURCE: &str = r#"
import radiance;

pub fn entry(ui: box<radiance.IUiHost>) -> int {
    let title: string = "妖弓编辑器";
    ui.window_centered(title, 200.0, 100.0, () => {
        ui.text(title);
        ui.dummy(0.0, 24.0);
    });
    0
}
"#;

const BUTTON_RETURN_SOURCE: &str = r#"
import radiance;

pub fn entry(ui: box<radiance.IUiHost>) -> bool {
    let pressed: bool = ui.button("go", 80.0, 24.0);
    pressed
}
"#;

const COLORED_TREE_SOURCE: &str = r#"
import radiance;

pub fn entry(ui: box<radiance.IUiHost>) -> bool {
    let clicked = ui.tree_leaf_colored("changed.pol", true, 0.35, 0.85, 0.45, 1.0);
    if ui.tree_node_open_colored("changed", 1.0, 0.62, 0.20, 1.0) {
        ui.tree_pop();
    }
    clicked
}
"#;

const ENABLED_AND_CLOSABLE_SOURCE: &str = r#"
import radiance;

pub fn entry(ui: box<radiance.IUiHost>) -> int {
    ui.menu_item_enabled("disabled menu", false, false);
    ui.button_enabled("disabled button", 90.0, 24.0, false);
    let closed = ui.window_centered_closable("Closable", 320.0, 200.0, () => {
        ui.text("inside closable");
    });
    if closed {
        return 1;
    }
    return 0;
}
"#;

#[test]
fn script_calls_ui_host_methods_and_invokes_closure_body() {
    let host = ScriptHost::new();
    host.load_source(NO_CAPTURE_SOURCE).expect("compile");

    let (recorder, ui_com) = RecordingUiHost::create();
    let com_id = host.intern(ui_com);
    let ui_box = host
        .foreign_box("radiance.comdef.IUiHost", com_id)
        .expect("ui foreign box");

    host.call_returning_data("entry", vec![ui_box])
        .expect("entry runs");

    let calls = recorder.calls.borrow().clone();
    assert_eq!(
        calls,
        vec![
            UiCall::WindowCentered {
                title: "hello".to_string(),
                w: 100.0,
                h: 50.0,
            },
            UiCall::BodyEnter("window_centered"),
            UiCall::Text("inside".to_string()),
            UiCall::BodyExit("window_centered"),
            UiCall::Text("after".to_string()),
        ]
    );
}

#[test]
fn sam_closure_captures_outer_let_bindings() {
    let host = ScriptHost::new();
    host.load_source(WITH_CAPTURE_SOURCE).expect("compile");

    let (recorder, ui_com) = RecordingUiHost::create();
    let com_id = host.intern(ui_com);
    let ui_box = host
        .foreign_box("radiance.comdef.IUiHost", com_id)
        .expect("ui foreign box");

    host.call_returning_data("entry", vec![ui_box])
        .expect("entry runs");

    let calls = recorder.calls.borrow().clone();
    assert_eq!(
        calls,
        vec![
            UiCall::WindowCentered {
                title: "妖弓编辑器".to_string(),
                w: 200.0,
                h: 100.0,
            },
            UiCall::BodyEnter("window_centered"),
            UiCall::Text("妖弓编辑器".to_string()),
            UiCall::Dummy { w: 0.0, h: 24.0 },
            UiCall::BodyExit("window_centered"),
        ]
    );
}

#[test]
fn button_return_value_propagates_to_script() {
    let host = ScriptHost::new();
    host.load_source(BUTTON_RETURN_SOURCE).expect("compile");

    let (recorder, ui_com) = RecordingUiHost::create();
    recorder
        .button_results
        .borrow_mut()
        .insert("go".to_string(), true);
    let com_id = host.intern(ui_com);
    let ui_box = host
        .foreign_box("radiance.comdef.IUiHost", com_id)
        .expect("ui foreign box");

    let result = host
        .call_returning_data("entry", vec![ui_box])
        .expect("entry runs");
    // Booleans cross the C ABI as int 0/1.
    assert_eq!(format!("{:?}", result), "Int(1)");

    assert_eq!(
        recorder.calls.borrow().clone(),
        vec![UiCall::Button {
            label: "go".to_string(),
            w: 80.0,
            h: 24.0,
        }]
    );
}

#[test]
fn unclicked_button_returns_zero() {
    let host = ScriptHost::new();
    host.load_source(BUTTON_RETURN_SOURCE).expect("compile");

    let (_recorder, ui_com) = RecordingUiHost::create();
    let com_id = host.intern(ui_com);
    let ui_box = host
        .foreign_box("radiance.comdef.IUiHost", com_id)
        .expect("ui foreign box");

    let result = host
        .call_returning_data("entry", vec![ui_box])
        .expect("entry runs");
    assert_eq!(format!("{:?}", result), "Int(0)");
}

#[test]
fn colored_tree_widgets_preserve_selection_and_color_arguments() {
    let host = ScriptHost::new();
    host.load_source(COLORED_TREE_SOURCE).expect("compile");

    let (recorder, ui_com) = RecordingUiHost::create();
    recorder
        .tree_leaf_results
        .borrow_mut()
        .insert("changed.pol".to_string(), true);
    recorder
        .tree_node_open_results
        .borrow_mut()
        .insert("changed".to_string(), true);
    let com_id = host.intern(ui_com);
    let ui_box = host
        .foreign_box("radiance.comdef.IUiHost", com_id)
        .expect("ui foreign box");

    let result = host
        .call_returning_data("entry", vec![ui_box])
        .expect("entry runs");
    assert_eq!(format!("{:?}", result), "Int(1)");
    assert_eq!(
        recorder.calls.borrow().clone(),
        vec![
            UiCall::TreeLeafColored {
                label: "changed.pol".to_string(),
                selected: true,
                color: [0.35, 0.85, 0.45, 1.0],
            },
            UiCall::TreeNodeOpenColored {
                label: "changed".to_string(),
                color: [1.0, 0.62, 0.20, 1.0],
            },
            UiCall::TreePop,
        ]
    );
}

#[test]
fn enabled_widgets_and_closable_window_propagate_state() {
    let host = ScriptHost::new();
    host.load_source(ENABLED_AND_CLOSABLE_SOURCE)
        .expect("compile");

    let (recorder, ui_com) = RecordingUiHost::create();
    recorder
        .menu_item_results
        .borrow_mut()
        .insert("disabled menu".to_string(), true);
    recorder
        .button_results
        .borrow_mut()
        .insert("disabled button".to_string(), true);
    recorder
        .window_close_results
        .borrow_mut()
        .insert("Closable".to_string(), true);
    let com_id = host.intern(ui_com);
    let ui_box = host
        .foreign_box("radiance.comdef.IUiHost", com_id)
        .expect("ui foreign box");

    let result = host
        .call_returning_data("entry", vec![ui_box])
        .expect("entry runs");
    assert_eq!(format!("{:?}", result), "Int(1)");
    assert_eq!(
        recorder.calls.borrow().clone(),
        vec![
            UiCall::MenuItemEnabled {
                label: "disabled menu".to_string(),
                selected: false,
                enabled: false,
            },
            UiCall::ButtonEnabled {
                label: "disabled button".to_string(),
                w: 90.0,
                h: 24.0,
                enabled: false,
            },
            UiCall::WindowCenteredClosable {
                title: "Closable".to_string(),
                w: 320.0,
                h: 200.0,
            },
            UiCall::BodyEnter("window_centered_closable"),
            UiCall::Text("inside closable".to_string()),
            UiCall::BodyExit("window_centered_closable"),
        ]
    );
}

/// Annotating a `bool`-returning IDL method's result as `int` must fail to
/// compile now that the bridge surface is first-class bool. Guards against
/// regressing to the old `IUiHost::button -> int` shape.
#[test]
fn bool_return_does_not_coerce_to_int() {
    let host = ScriptHost::new();
    let src = r#"
import radiance;

pub fn entry(ui: box<radiance.IUiHost>) -> int {
    let pressed: int = ui.button("nope", 80.0, 24.0);
    pressed
}
"#;
    let err = host
        .load_source(src)
        .expect_err("expected a type-check failure");
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("bool") && msg.contains("int"),
        "expected bool/int mismatch, got: {msg}",
    );
}
