//! Smoke test for the `radiance_scripting.ui` declarative widget layer.
//!
//! Mounts `scripts/ui.p7` as a binding, loads a tiny screen that builds a
//! `Column` of a `Text` and a `Button`, and paints it through a
//! `RecordingUiHost` with a seeded logical viewport + text metrics. The
//! recorded `IUiHost` calls prove that:
//!
//!   1. The declarative tree lowers to absolute-positioned host draws
//!      (`text_at`, `button`) at the layout engine's computed positions.
//!   2. Bare widget values in a `[...]` children literal auto-box to
//!      `box<Element>` (no explicit `box(...)`).
//!   3. A `Button`'s `on_click` closure field fires when the host reports
//!      a click — observable here because the callback issues its own
//!      `ui.text("clicked")` host call.

use radiance_scripting::ScriptHost;
use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};

const UI_MODULE: &str = include_str!("../scripts/ui.p7");

const SCREEN: &str = r#"
import radiance;
import radiance_scripting.ui;

pub fn entry(host: box<radiance.IUiHost>) -> int {
    let ctx = ui.make_ctx();
    let items: array<box<ui.Element>> = [
        ui.Text("a"),
        ui.Button("go", 100.0, 20.0, () => { host.text("clicked"); }),
    ];
    let root: box<ui.Element> = ui.Column(items, 0.0);
    ui.paint_root(root, host, ctx)
}
"#;

fn run(ui_com: crosscom::ComRc<radiance::comdef::IUiHost>) {
    let host = ScriptHost::new();
    host.add_binding("radiance_scripting.ui", UI_MODULE);
    host.load_source(SCREEN).expect("screen compiles");
    let com_id = host.intern(ui_com);
    let ui_box = host
        .foreign_box("radiance.comdef.IUiHost", com_id)
        .expect("ui foreign box");
    host.call_returning_data("entry", vec![ui_box])
        .expect("entry runs");
}

#[test]
fn column_lays_out_children_and_button_callback_fires() {
    let (recorder, ui_com) = RecordingUiHost::create();
    recorder.set_display_size(200, 100);
    recorder.set_char_size(10.0, 16.0);
    recorder.button_results.borrow_mut().insert("go".into(), true);

    run(ui_com);

    let calls = recorder.calls.borrow().clone();

    // Text "a" measures 10x16; centred in width 200 -> x = (200-10)/2 = 95,
    // painted at the column's top (y = 0).
    assert!(
        calls.iter().any(|c| matches!(
            c,
            UiCall::TextAt { x, y, s, .. }
                if (*x - 95.0).abs() < 0.01 && (*y - 0.0).abs() < 0.01 && s == "a"
        )),
        "expected Text 'a' at (95, 0); got {calls:?}"
    );

    // Button measures 100x20; centred -> x = 50, stacked below the 16px
    // text. (Position is via set_cursor_pos which the recorder does not
    // log, so we assert the Button submission itself.)
    assert!(
        calls.iter().any(|c| matches!(
            c,
            UiCall::Button { label, w, h }
                if label == "go" && (*w - 100.0).abs() < 0.01 && (*h - 20.0).abs() < 0.01
        )),
        "expected Button 'go' 100x20; got {calls:?}"
    );

    // Clicking fired the callback, which issued its own text() call.
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s == "clicked")),
        "expected on_click callback to fire (Text 'clicked'); got {calls:?}"
    );
}

#[test]
fn unclicked_button_does_not_fire_callback() {
    let (recorder, ui_com) = RecordingUiHost::create();
    recorder.set_display_size(200, 100);
    recorder.set_char_size(10.0, 16.0);
    // No button result seeded -> button() returns false.

    run(ui_com);

    let calls = recorder.calls.borrow().clone();
    assert!(
        !calls
            .iter()
            .any(|c| matches!(c, UiCall::Text(s) if s == "clicked")),
        "callback must not fire when the button is not clicked; got {calls:?}"
    );
}
