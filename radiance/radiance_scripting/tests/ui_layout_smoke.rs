//! Smoke for the declarative layout/builder widgets added for the
//! composition-only authoring pass: `ListView` (builds rows from an index
//! closure passed screen->lib), `FittedBox` (letterbox), and `Align`. These
//! paths back the PAL4 load screen, which the agent server can't reach
//! (no mouse), so they are pinned here against a `RecordingUiHost`.

use radiance_scripting::ScriptHost;
use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};

const UI_MODULE: &str = include_str!("../scripts/ui.p7");

fn run(src: &str, recorder: &RecordingUiHost, ui_com: crosscom::ComRc<radiance::comdef::IUiHost>) {
    let _ = recorder;
    let host = ScriptHost::new();
    host.add_binding("radiance_scripting.ui", UI_MODULE);
    host.load_source(src).expect("screen compiles");
    let com_id = host.intern(ui_com);
    let ui_box = host
        .foreign_box("radiance.comdef.IUiHost", com_id)
        .expect("ui foreign box");
    host.call_returning_data("entry", vec![ui_box]).expect("entry runs");
}

// `ListView(3, builder)` over a 300px-tall viewport => 3 rows of 100px each
// at y = 0, 100, 200. The builder (a screen closure, invoked inside the lib
// widget) returns a `Text` whose `paint` issues `text_at(x, y, ...)`, so the
// recorded y-coordinates prove per-row layout + cross-module builder firing.
const LISTVIEW_SCREEN: &str = r#"
import radiance;
import radiance_scripting.ui;

fn row(i: int) -> box<ui.Element> {
    return ui.Text("r");
}

pub fn entry(host: box<radiance.IUiHost>) -> int {
    let ctx = ui.make_ctx();
    let root: box<ui.Element> = ui.ListView(3, (i: int) => row(i), 0.0);
    ui.paint_root(root, host, ctx)
}
"#;

#[test]
fn listview_builds_and_positions_rows() {
    let (recorder, ui_com) = RecordingUiHost::create();
    recorder.set_display_size(200, 300);
    recorder.set_char_size(10.0, 16.0);

    run(LISTVIEW_SCREEN, &recorder, ui_com);

    let calls = recorder.calls.borrow();
    let ys: Vec<f32> = calls
        .iter()
        .filter_map(|c| match c {
            UiCall::TextAt { y, s, .. } if s == "r" => Some(*y),
            _ => None,
        })
        .collect();
    assert_eq!(ys.len(), 3, "ListView must build 3 rows; got {calls:?}");
    assert!((ys[0] - 0.0).abs() < 0.01, "row 0 at y=0; got {ys:?}");
    assert!((ys[1] - 100.0).abs() < 0.01, "row 1 at y=100; got {ys:?}");
    assert!((ys[2] - 200.0).abs() < 0.01, "row 2 at y=200; got {ys:?}");
}

// `FittedBox(800, 600, child)` into a 1600x600 viewport letterboxes to
// scale=1.0 (limited by height), drawn_w=800, centred => origin_x=400. An
// `Align(child, 0.0, 0.0)` inside paints the child (a Text) at the fitted
// box's top-left, i.e. x=400, y=0.
const FITTED_ALIGN_SCREEN: &str = r#"
import radiance;
import radiance_scripting.ui;

pub fn entry(host: box<radiance.IUiHost>) -> int {
    let ctx = ui.make_ctx();
    let inner: box<ui.Element> = ui.Align(ui.Text("x"), 0.0, 0.0);
    let root: box<ui.Element> = ui.FittedBox(800.0, 600.0, inner);
    ui.paint_root(root, host, ctx)
}
"#;

#[test]
fn fittedbox_letterboxes_and_align_places_child() {
    let (recorder, ui_com) = RecordingUiHost::create();
    recorder.set_display_size(1600, 600);
    recorder.set_char_size(10.0, 16.0);

    run(FITTED_ALIGN_SCREEN, &recorder, ui_com);

    let calls = recorder.calls.borrow();
    assert!(
        calls.iter().any(|c| matches!(
            c,
            UiCall::TextAt { x, y, s, .. }
                if (*x - 400.0).abs() < 0.01 && (*y - 0.0).abs() < 0.01 && s == "x"
        )),
        "FittedBox+Align(top-left) should place text at (400, 0); got {calls:?}"
    );
}
