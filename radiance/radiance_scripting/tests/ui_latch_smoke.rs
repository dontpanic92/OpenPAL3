//! Smoke test for the cross-frame press latch of the `radiance_scripting.ui`
//! interactive widgets (`LayoutImage` / `ImageButton` / `TextButton`), which
//! drive their own hover/press state through `UiCtx` instead of relying on
//! imgui's native button. This is the path the migrated PAL4 start menu uses
//! for every layout button and save-slot row, and it cannot be exercised
//! through the agent server (which has no mouse input), so it is pinned here.
//!
//! It also guards the `paint_root` latch ordering: the latch must be cleared
//! at the END of the frame (after widgets run), so a widget can still observe
//! `was == key` on the release frame and fire its `on_click`. Clearing first
//! (the old `begin_frame` behaviour) would swallow every click.

use radiance_scripting::ScriptHost;
use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};

const UI_MODULE: &str = include_str!("../scripts/ui.p7");

// A single interactive `layout_button` painted twice with the same `ctx`.
// `mouse_down` is scripted as press (frame 1) then release (frame 2); the
// `on_click` callback issues `host.text("clicked")` so the recorder can see
// exactly when it fires.
const SCREEN: &str = r#"
import radiance;
import radiance_scripting.ui;

pub fn entry(host: box<radiance.IUiHost>) -> int {
    let ctx = ui.make_ctx();
    let btn: box<ui.Element> = ui.LayoutImage(
        7, 100, 0.0, 0.0, 1.0, 1.0, true,
        () => { host.text("clicked"); });
    let items: array<box<ui.Element>> = [ui.Positioned(0.0, 0.0, 50.0, 50.0, btn)];
    let root: box<ui.Element> = ui.Stack(items);
    ui.paint_root(root, host, ctx);
    ui.paint_root(root, host, ctx);
    0
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

fn clicked_count(calls: &[UiCall]) -> usize {
    calls
        .iter()
        .filter(|c| matches!(c, UiCall::Text(s) if s == "clicked"))
        .count()
}

#[test]
fn press_then_release_over_widget_fires_once() {
    let (recorder, ui_com) = RecordingUiHost::create();
    recorder.set_display_size(200, 100);
    recorder.set_item_hovered(true);
    // Per frame: widget calls mouse_down once, then paint_root's end_frame
    // calls it once. Two frames -> [press, press, release, release].
    recorder.set_mouse_down_script([true, true, false, false]);

    run(ui_com);

    let calls = recorder.calls.borrow();
    assert_eq!(
        clicked_count(&calls),
        1,
        "a press over the widget followed by a release over it must fire \
         on_click exactly once; got {calls:?}"
    );
}

#[test]
fn release_off_widget_does_not_fire() {
    let (recorder, ui_com) = RecordingUiHost::create();
    recorder.set_display_size(200, 100);
    // Press while hovered (frame 1), then release while NOT hovered
    // (frame 2): the click must be cancelled.
    recorder.set_mouse_down_script([true, true, false, false]);
    // hovered = false for the whole run -> the press is never latched, so
    // there is nothing to release-fire even though the button "went up".
    recorder.set_item_hovered(false);

    run(ui_com);

    let calls = recorder.calls.borrow();
    assert_eq!(
        clicked_count(&calls),
        0,
        "a release that is not over the pressed widget must not fire; \
         got {calls:?}"
    );
}

/// Regression for the `make_ctx()`-per-frame footgun.
///
/// The press latch used to be a plain field on the script-side `UiCtx`. Five
/// shipped screens (`title.p7`, `openpal4/loading_overlay.p7`,
/// `openpal3/status_ui.p7`, and `openpal3/dlg_box.p7` twice) call `make_ctx()`
/// *inside* their per-frame `render`, which silently rebuilt the latch every
/// frame: a press on frame N was forgotten before the release on frame N+1
/// could observe it, so `on_click` could never fire. Those screens only worked
/// because they happened to use imgui's native `button` rather than a
/// latch-driven widget.
///
/// The latch now lives on the host (`IUiHost.press_latch`), so a fresh `UiCtx`
/// per frame is harmless. This test builds the context inside the "frame" —
/// exactly the pattern that used to be broken — and asserts the click still
/// fires.
const SCREEN_FRESH_CTX_PER_FRAME: &str = r#"
import radiance;
import radiance_scripting.ui;

// One "frame": build a brand-new ctx, then paint. Mirrors a screen that calls
// `gui.make_ctx()` inside `render`.
fn frame(host: box<radiance.IUiHost>) {
    let ctx = ui.make_ctx();
    let btn: box<ui.Element> = ui.LayoutImage(
        7, 100, 0.0, 0.0, 1.0, 1.0, true,
        () => { host.text("clicked"); });
    let items: array<box<ui.Element>> = [ui.Positioned(0.0, 0.0, 50.0, 50.0, btn)];
    let root: box<ui.Element> = ui.Stack(items);
    ui.paint_root(root, host, ctx);
}

pub fn entry(host: box<radiance.IUiHost>) -> int {
    frame(host);
    frame(host);
    0
}
"#;

#[test]
fn latch_survives_a_rebuilt_context_each_frame() {
    let (recorder, ui_com) = RecordingUiHost::create();
    recorder.set_display_size(200, 100);
    recorder.set_item_hovered(true);
    recorder.set_mouse_down_script([true, true, false, false]);

    let host = ScriptHost::new();
    host.add_binding("radiance_scripting.ui", UI_MODULE);
    host.load_source(SCREEN_FRESH_CTX_PER_FRAME)
        .expect("screen compiles");
    let com_id = host.intern(ui_com);
    let ui_box = host
        .foreign_box("radiance.comdef.IUiHost", com_id)
        .expect("ui foreign box");
    host.call_returning_data("entry", vec![ui_box])
        .expect("entry runs");

    let calls = recorder.calls.borrow();
    assert_eq!(
        clicked_count(&calls),
        1,
        "the press latch must live on the host, so rebuilding UiCtx every \
         frame cannot swallow a click; got {calls:?}"
    );
}

/// The latch sentinel is `-1`, which means it round-trips a **negative** value
/// through the crosscom -> p7 return path. IDL `int` is `c_int` (32-bit) while
/// the CIF reads the return register as the wider `c_long`, so without an
/// explicit narrowing through `c_int` the high half is zero-extended and `-1`
/// arrives in script as `4294967295` — making every `== NO_PRESS` comparison
/// false and breaking the latch in a way that looks like a UI bug.
///
/// This pins the sign extension directly, independent of the widget layer.
#[test]
fn negative_ints_round_trip_from_the_host() {
    let (recorder, ui_com) = RecordingUiHost::create();
    let host = ScriptHost::new();
    host.add_binding("radiance_scripting.ui", UI_MODULE);
    host.load_source(
        r#"
import radiance;
pub fn entry(host: box<radiance.IUiHost>) -> int {
    host.set_press_latch(-1);
    host.text(f"a={host.press_latch()}");
    host.set_press_latch(-2147483648);
    host.text(f"b={host.press_latch()}");
    host.set_press_latch(2147483647);
    host.text(f"c={host.press_latch()}");
    0
}
"#,
    )
    .expect("screen compiles");
    let com_id = host.intern(ui_com);
    let ui_box = host
        .foreign_box("radiance.comdef.IUiHost", com_id)
        .expect("ui foreign box");
    host.call_returning_data("entry", vec![ui_box])
        .expect("entry runs");

    let calls = recorder.calls.borrow();
    let texts: Vec<String> = calls
        .iter()
        .filter_map(|c| match c {
            UiCall::Text(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts,
        vec![
            "a=-1".to_string(),
            "b=-2147483648".to_string(),
            "c=2147483647".to_string(),
        ],
        "IDL `int` must sign-extend on the way back into script"
    );
}
