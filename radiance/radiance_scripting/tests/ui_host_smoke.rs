//! H2 smoke test (per generated/plan.md): end-to-end SAM + crosscom +
//! p7 plumbing through an `IUiHost` host service.
//!
//! Each test loads a tiny p7 module that declares a script struct with
//! a `render(ui: &IUiHost, dt: float)` style entry point. The
//! Rust test wires a `RecordingUiHost` ComObject, hands the script a
//! `&IUiHost` foreign value, and verifies that:
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

pub fn entry(ui: &radiance.IUiHost) -> int {
    ui.window_centered("hello", 100.0, 50.0, () => {
        ui.text("inside");
    });
    ui.text("after");
    0
}
"#;

#[test]
fn rotated_sprite_preserves_pivot_angle_and_atlas_uv_across_script_bridge() {
    let host = ScriptHost::new();
    host.load_source(
        r#"
        import radiance;
        pub fn entry(ui: &radiance.IUiHost) {
            ui.image_rotated(-17, 170.0, 83.0, 502.0, 519.0,
                342.0, 591.0, -0.5, 0.125, 0.25, 0.75, 0.875);
        }
    "#,
    )
    .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    let id = host.intern(ui);
    host.call_void(
        "entry",
        vec![host.foreign_box("radiance.comdef.IUiHost", id).unwrap()],
    )
    .unwrap();
    assert_eq!(
        *recorder.calls.borrow(),
        vec![UiCall::RotatedImage {
            texture_com_id: -17,
            rect: [170., 83., 502., 519.],
            pivot: [342., 591.],
            radians: -0.5,
            uv: [0.125, 0.25, 0.75, 0.875],
        }]
    );
}

const WITH_CAPTURE_SOURCE: &str = r#"
import radiance;

pub fn entry(ui: &radiance.IUiHost) -> int {
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

pub fn entry(ui: &radiance.IUiHost) -> bool {
    let pressed: bool = ui.button("go", 80.0, 24.0);
    pressed
}
"#;

const COLORED_TREE_SOURCE: &str = r#"
import radiance;

pub fn entry(ui: &radiance.IUiHost) -> bool {
    let clicked = ui.tree_leaf_colored("changed.pol", true, 0.35, 0.85, 0.45, 1.0);
    if ui.tree_node_open_colored("changed", 1.0, 0.62, 0.20, 1.0) {
        ui.tree_pop();
    }
    clicked
}
"#;

const ENABLED_AND_CLOSABLE_SOURCE: &str = r#"
import radiance;

pub fn entry(ui: &radiance.IUiHost) -> int {
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

pub fn entry(ui: &radiance.IUiHost) -> int {
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

#[test]
fn background_canvas_clipping_does_not_activate_the_implicit_debug_window() {
    use radiance_scripting::services::ui_host::{ImguiFrameState, ImguiUiHost, with_imgui_frame};

    struct NoTextures;
    impl radiance_scripting::TextureResolver for NoTextures {
        fn resolve(&mut self, _: i64) -> Option<imgui::TextureId> {
            None
        }
    }

    let host = ScriptHost::new();
    host.load_source(
        r#"
        import radiance;
        pub fn frame(ui: &radiance.IUiHost) {
            ui.clip_rect(10.0, 20.0, 110.0, 120.0, () => {
                ui.clip_rect(20.0, 30.0, 100.0, 110.0, () => {
                    ui.fill_rect(0.0, 0.0, 200.0, 200.0, 1.0, 1.0, 1.0, 1.0);
                    ui.text_at_size(30.0, 40.0, 20.0, 1.0, 1.0, 1.0, 1.0, "dialog");
                });
            });
        }
        "#,
    )
    .unwrap();
    let id = host.intern(ImguiUiHost::create());
    let ui_box = host.foreign_box("radiance.comdef.IUiHost", id).unwrap();
    let mut context = imgui::Context::create();
    context.set_ini_filename(None);
    context.io_mut().display_size = [800.0, 600.0];
    context.io_mut().delta_time = 1.0 / 60.0;
    context.fonts().build_rgba32_texture();
    context.fonts().tex_id = imgui::TextureId::new(1);
    let fonts = context.fonts().fonts().to_vec();
    let mut textures = NoTextures;

    for in_window in [false, true] {
        for dpi in [1.0, 2.0] {
            let ui = context.frame();
            let mut frame = ImguiFrameState::new(ui, &mut textures, &fonts, dpi);
            let mut draw = || {
                with_imgui_frame(&mut frame, || {
                    host.call_void("frame", vec![ui_box.clone()]).unwrap();
                });
            };
            if in_window {
                ui.window("Explicit")
                    .position([0.0, 0.0], imgui::Condition::Always)
                    .size([400.0, 300.0], imgui::Condition::Always)
                    .build(draw);
            } else {
                draw();
            }
            let data = context.render();
            assert!(data.total_vtx_count > 0);
            assert!(data.draw_lists().any(|list| {
                list.commands().any(|command| {
                    matches!(command, imgui::DrawCmd::Elements { cmd_params, .. }
                        if cmd_params.clip_rect == [20.0 * dpi, 30.0 * dpi, 100.0 * dpi, 110.0 * dpi])
                })
            }));
            assert_eq!(
                context.io().metrics_render_windows,
                i32::from(in_window),
                "background-only drawing must not expose ImGui's Debug window"
            );
        }
    }
}
