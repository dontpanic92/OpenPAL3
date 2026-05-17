//! End-to-end smoke test for the protosept-authored PAL4 debug overlay.
//!
//! Verifies that:
//! 1. `pal4_debug_main.p7` + `pal4_debug_overlay.p7` parse and compile
//!    against the generated `pal4_debug.p7` binding.
//! 2. `wrap_overlay` produces a working `ComRc<IPal4DebugOverlay>`.
//! 3. Calling `overlay.render(ui, dt, ctx)` from Rust dispatches into
//!    the script struct and reaches the `IUiHost` it received.
//! 4. The script reads back `ctx.scene_name()` / etc. via the COM
//!    boundary, matching the snapshot the host pushed in.
//!
//! Mirrors the editor's `wrap_director_smoke` / `ui_host_smoke` recipe.

use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};
use radiance_scripting::{with_services, RuntimeAccess, ScriptHost};
use shared::openpal4::debug::{create_context, wrap_overlay, Pal4DebugSnapshot, PAL4_DEBUG_P7};

const MAIN_P7: &str = include_str!("../../yaobow/scripts/pal4_debug_main.p7");
const OVERLAY_P7: &str = include_str!("../../yaobow/scripts/pal4_debug_overlay.p7");

#[test]
fn pal4_debug_overlay_round_trips_through_p7() {
    let host = ScriptHost::new();
    host.add_binding("pal4_debug", PAL4_DEBUG_P7);
    host.add_binding("pal4_debug_overlay", OVERLAY_P7);
    host.load_source(MAIN_P7).expect("pal4_debug_main.p7 must compile");

    // Host-side debug context with a programmed snapshot the script
    // should echo back to us through `ctx.scene_name()` & friends.
    let (ctx_inner, ctx_com) = create_context();
    ctx_inner.set_snapshot(Pal4DebugSnapshot {
        scene_name: "Q01".into(),
        block_name: "q01".into(),
        leader_index: 2,
        leader_pos: [10.0, 20.0, 30.0],
        delta_time: 0.016,
        fps: 60.0,
    });

    // Call `init(ctx)` — this returns the script-built overlay box.
    let ctx_handle = host.intern(ctx_com.clone());
    let ctx_box = host
        .foreign_box(
            "shared.openpal4.comdef.pal4_debug.IPal4DebugContext",
            ctx_handle,
        )
        .expect("IPal4DebugContext foreign_box must build");
    let overlay_data = host
        .call_returning_data("init", vec![ctx_box])
        .expect("init() must succeed");

    // Reverse-wrap via the runtime-typed CCW factory.
    let runtime_handle = {
        let mut out = None;
        <ScriptHost as RuntimeAccess>::with_ctx(&host, &mut |_ctx| {
            let h = with_services(|s| s.runtime_handle())
                .expect("with_services in scope");
            out = Some(h);
        });
        out.expect("RuntimeAccess ran body")
    };
    let overlay = wrap_overlay(&runtime_handle, overlay_data)
        .expect("wrap_overlay must succeed");

    // Drive a render with a recording UI host so we can assert what
    // the script emitted to imgui.
    let (recorder, ui_com) = RecordingUiHost::create();
    overlay.render(ui_com, 0.016, ctx_com);

    let calls = recorder.calls.borrow().clone();
    // First call must be the outer window. The body is invoked
    // between BodyEnter/BodyExit markers by RecordingUiHost.
    assert!(matches!(calls.first(), Some(UiCall::Window { .. })),
            "first call should be ui.window, got {:?}", calls.first());

    // Body must have produced text calls echoing the snapshot fields.
    let texts: Vec<String> = calls
        .iter()
        .filter_map(|c| match c {
            UiCall::Text(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|t| t.contains("Q01")),
            "expected scene_name in text calls, got {:?}", texts);
    assert!(texts.iter().any(|t| t.contains("q01")),
            "expected block_name in text calls, got {:?}", texts);
    assert!(texts.iter().any(|t| t.contains("#2")),
            "expected leader_index in text calls, got {:?}", texts);
}
