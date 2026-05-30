//! End-to-end smoke test for the protosept-authored PAL4 debug overlay.
//!
//! Verifies that:
//! 1. `pal4_debug_overlay.p7` parses and compiles against the generated
//!    `pal4_debug.p7` binding.
//! 2. `wrap_overlay` produces a working `ComRc<IPal4DebugOverlay>`.
//! 3. Calling `overlay.render(ui, dt, ctx)` from Rust dispatches into
//!    the script struct and reaches the `IUiHost` it received.
//! 4. The script reads back `ctx.scene_name()` / etc. via the COM
//!    boundary, matching the snapshot the host pushed in.
//!
//! Mirrors the editor's `wrap_director_smoke` / `ui_host_smoke` recipe.

use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};
use radiance_scripting::{with_services, RuntimeAccess, ScriptHost};
use shared::openpal4::pal4_debug::{
    create_debug_session, wrap_overlay, Pal4DebugSnapshot, PAL4_DEBUG_P7,
};

const OVERLAY_P7: &str = include_str!("../../yaobow/scripts/pal4_debug_overlay.p7");

#[test]
fn pal4_debug_overlay_round_trips_through_p7() {
    let host = ScriptHost::new();
    host.add_binding("pal4_debug", PAL4_DEBUG_P7);
    host.load_source(OVERLAY_P7)
        .expect("pal4_debug_overlay.p7 must compile");

    // Host-side debug context with a programmed snapshot the script
    // should echo back to us through `ctx.scene_name()` & friends.
    let session = create_debug_session();
    session.state.set_snapshot(Pal4DebugSnapshot {
        scene_name: "Q01".into(),
        block_name: "q01".into(),
        leader_index: 2,
        leader_pos: [10.0, 20.0, 30.0],
        delta_time: 0.016,
        fps: 60.0,
    });

    // Call `make_overlay(ctx)` — this returns the script-built overlay box.
    let ctx_handle = host.intern(session.context.clone());
    let ctx_box = host
        .foreign_box(
            "shared.openpal4.comdef.pal4_debug.IPal4DebugContext",
            ctx_handle,
        )
        .expect("IPal4DebugContext foreign_box must build");
    let overlay_data = host
        .call_returning_data("make_overlay", vec![ctx_box])
        .expect("make_overlay() must succeed");

    // Reverse-wrap via the runtime-typed CCW factory.
    let runtime_handle = {
        let mut out = None;
        <ScriptHost as RuntimeAccess>::with_ctx(&host, &mut |_ctx| {
            let h = with_services(|s| s.runtime_handle()).expect("with_services in scope");
            out = Some(h);
        });
        out.expect("RuntimeAccess ran body")
    };
    let overlay = wrap_overlay(&runtime_handle, overlay_data).expect("wrap_overlay must succeed");

    // Drive a render with a recording UI host so we can assert what
    // the script emitted to imgui.
    let (recorder, ui_com) = RecordingUiHost::create();
    overlay.render(ui_com, 0.016, session.context);

    let calls = recorder.calls.borrow().clone();
    // The script wraps its window in `ui.with_font(1, ...)`, so the
    // first recorded call is the font scope; the window opens
    // inside its body. Assert the expected nesting: WithFont first,
    // a Window somewhere in the recorded sequence, and the Window
    // sits inside the with_font body (i.e. between
    // BodyEnter("with_font") and BodyExit("with_font")).
    assert!(
        matches!(calls.first(), Some(UiCall::WithFont { font_idx: 1 })),
        "first call should be ui.with_font(1, ...), got {:?}",
        calls.first()
    );
    let with_font_enter = calls
        .iter()
        .position(|c| matches!(c, UiCall::BodyEnter("with_font")))
        .expect("BodyEnter('with_font') must appear");
    let with_font_exit = calls
        .iter()
        .position(|c| matches!(c, UiCall::BodyExit("with_font")))
        .expect("BodyExit('with_font') must appear");
    let window_pos = calls
        .iter()
        .position(|c| matches!(c, UiCall::Window { .. }))
        .expect("ui.window call must appear inside the with_font scope");
    assert!(
        window_pos > with_font_enter && window_pos < with_font_exit,
        "ui.window must be nested inside the with_font body; saw window at {} but with_font spans [{}, {}]",
        window_pos,
        with_font_enter,
        with_font_exit
    );

    // Body must have produced text calls echoing the snapshot fields.
    let texts: Vec<String> = calls
        .iter()
        .filter_map(|c| match c {
            UiCall::Text(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    assert!(
        texts.iter().any(|t| t.contains("Q01")),
        "expected scene_name in text calls, got {:?}",
        texts
    );
    assert!(
        texts.iter().any(|t| t.contains("q01")),
        "expected block_name in text calls, got {:?}",
        texts
    );
    assert!(
        texts.iter().any(|t| t.contains("#2")),
        "expected leader_index in text calls, got {:?}",
        texts
    );

    // Toggle buttons must appear, with labels reflecting the default
    // host-side state (BSP on, nav-mesh off).
    let buttons: Vec<String> = calls
        .iter()
        .filter_map(|c| match c {
            UiCall::Button { label, .. } => Some(label.clone()),
            _ => None,
        })
        .collect();
    assert!(
        buttons
            .iter()
            .any(|b| b.starts_with("BSP:") && b.contains("ON")),
        "expected BSP toggle button defaulting to ON, got {:?}",
        buttons
    );
    assert!(
        buttons
            .iter()
            .any(|b| b.starts_with("NavMesh:") && b.contains("OFF")),
        "expected NavMesh toggle button defaulting to OFF, got {:?}",
        buttons
    );
    assert!(
        buttons
            .iter()
            .any(|b| b.starts_with("FastFwd:") && b.contains("OFF")),
        "expected FastFwd toggle button defaulting to OFF, got {:?}",
        buttons
    );
}

/// The script-side toggle buttons must round-trip through
/// `IPal4DebugContext` to mutate the host's `Pal4DebugState`
/// flags. Pre-seed `RecordingUiHost::button_results` with the exact
/// labels we expect the script to emit while at default state (BSP
/// on, NavMesh off) and assert the flags flipped after `render`.
#[test]
fn pal4_debug_overlay_toggles_flip_host_state() {
    let host = ScriptHost::new();
    host.add_binding("pal4_debug", PAL4_DEBUG_P7);
    host.load_source(OVERLAY_P7)
        .expect("pal4_debug_overlay.p7 must compile");

    let session = create_debug_session();

    let ctx_handle = host.intern(session.context.clone());
    let ctx_box = host
        .foreign_box(
            "shared.openpal4.comdef.pal4_debug.IPal4DebugContext",
            ctx_handle,
        )
        .expect("IPal4DebugContext foreign_box must build");
    let overlay_data = host
        .call_returning_data("make_overlay", vec![ctx_box])
        .expect("make_overlay() must succeed");

    let runtime_handle = {
        let mut out = None;
        <ScriptHost as RuntimeAccess>::with_ctx(&host, &mut |_ctx| {
            let h = with_services(|s| s.runtime_handle()).expect("with_services in scope");
            out = Some(h);
        });
        out.expect("RuntimeAccess ran body")
    };
    let overlay = wrap_overlay(&runtime_handle, overlay_data).expect("wrap_overlay must succeed");

    // Press the BSP button only. Default: BSP on (label "ON") →
    // expect bsp_visible to flip to false after this render.
    let (recorder, ui_com) = RecordingUiHost::create();
    recorder
        .button_results
        .borrow_mut()
        .insert("BSP:      ON".into(), true);
    overlay.render(ui_com, 0.016, session.context.clone());
    assert!(
        !session.state.bsp_visible(),
        "BSP flag should have been toggled to false"
    );
    assert!(
        !session.state.nav_mesh_visible(),
        "NavMesh flag should remain false (no press)"
    );

    // Now press only NavMesh. State at start of this render: BSP off
    // (post previous toggle), NavMesh off. Label: "NavMesh:  OFF".
    let (recorder2, ui_com2) = RecordingUiHost::create();
    recorder2
        .button_results
        .borrow_mut()
        .insert("NavMesh:  OFF".into(), true);
    overlay.render(ui_com2, 0.016, session.context.clone());
    assert!(
        !session.state.bsp_visible(),
        "BSP flag should stay false after NavMesh press"
    );
    assert!(
        session.state.nav_mesh_visible(),
        "NavMesh flag should have been toggled to true"
    );

    // Finally press only FastFwd. State: fast-forward off by default →
    // expect it to flip to true after this render.
    let (recorder3, ui_com3) = RecordingUiHost::create();
    recorder3
        .button_results
        .borrow_mut()
        .insert("FastFwd:  OFF".into(), true);
    overlay.render(ui_com3, 0.016, session.context);
    assert!(
        session.state.fast_forward(),
        "FastFwd flag should have been toggled to true"
    );
}
