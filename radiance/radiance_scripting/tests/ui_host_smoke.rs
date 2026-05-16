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

use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};
use radiance_scripting::ScriptHost;

const NO_CAPTURE_SOURCE: &str = r#"
import immediate_director;

pub fn entry(ui: box<immediate_director.IUiHost>) -> int {
    ui.window_centered("hello", 100.0, 50.0, () => {
        ui.text("inside");
    });
    ui.text("after");
    0
}
"#;

const WITH_CAPTURE_SOURCE: &str = r#"
import immediate_director;

pub fn entry(ui: box<immediate_director.IUiHost>) -> int {
    let title: string = "妖弓编辑器";
    ui.window_centered(title, 200.0, 100.0, () => {
        ui.text(title);
        ui.dummy(0.0, 24.0);
    });
    0
}
"#;

const BUTTON_RETURN_SOURCE: &str = r#"
import immediate_director;

pub fn entry(ui: box<immediate_director.IUiHost>) -> int {
    let pressed: int = ui.button("go", 80.0, 24.0);
    pressed
}
"#;

#[test]
fn script_calls_ui_host_methods_and_invokes_closure_body() {
    let host = ScriptHost::new();
    host.load_source(NO_CAPTURE_SOURCE).expect("compile");

    let (recorder, ui_com) = RecordingUiHost::create();
    let com_id = host.intern(ui_com);
    let ui_box = host
        .foreign_box("radiance_scripting.comdef.immediate_director.IUiHost", com_id)
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
        .foreign_box("radiance_scripting.comdef.immediate_director.IUiHost", com_id)
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
        .foreign_box("radiance_scripting.comdef.immediate_director.IUiHost", com_id)
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
        .foreign_box("radiance_scripting.comdef.immediate_director.IUiHost", com_id)
        .expect("ui foreign box");

    let result = host
        .call_returning_data("entry", vec![ui_box])
        .expect("entry runs");
    assert_eq!(format!("{:?}", result), "Int(0)");
}
