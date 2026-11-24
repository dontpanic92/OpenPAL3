//! Compile-only smoke for the declarative `openpal3/dlg_box.p7`, migrated
//! onto the `radiance_scripting.ui` widget framework (the 9-slice chrome,
//! wrapped CJK text grid and fade curtain are screen-local `gui.Element`
//! leaves). Mounts the real script asset bundle and loads the script
//! through the production VFS module provider so
//! `import radiance_scripting.ui;` resolves.

use crosscom::ComRc;
use p7::interpreter::context::Data;
use radiance_scripting::ScriptHost;
use radiance_scripting::{
    comdef::services::{IAtlasPage, ISprite, ISpriteImpl, ISpriteService, ISpriteServiceImpl},
    services::ui_host_recording::{RecordingUiHost, UiCall},
};
use yaobow_lib::script_source::install_script_assets;

mod comdef {
    pub use radiance_scripting::comdef::*;
}

struct TestSprite {
    id: i32,
    width: i32,
    height: i32,
}
radiance_scripting::ComObject_Sprite!(crate::TestSprite);
impl ISpriteImpl for TestSprite {
    fn com_id(&self) -> i32 {
        self.id
    }
    fn width(&self) -> i32 {
        self.width
    }
    fn height(&self) -> i32 {
        self.height
    }
    fn u0(&self) -> f32 {
        0.125
    }
    fn v0(&self) -> f32 {
        0.25
    }
    fn u1(&self) -> f32 {
        0.75
    }
    fn v1(&self) -> f32 {
        0.875
    }
}

struct TestSprites;
radiance_scripting::ComObject_SpriteService!(crate::TestSprites);
impl ISpriteServiceImpl for TestSprites {
    fn load_sprite(&self, path: &str) -> Option<ComRc<ISprite>> {
        let (id, width, height) = if path.contains("/ROLE/") {
            (77, 256, 512)
        } else if path.contains("/page") {
            (88, 32, 32)
        } else {
            (1, 16, 16)
        };
        Some(ComRc::from_object(TestSprite { id, width, height }))
    }
    fn load_atlas_page(&self, _: &str) -> Option<ComRc<IAtlasPage>> {
        None
    }
}

#[test]
fn pal3_dlg_box_compiles_on_ui_framework() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source_from_path("/yaobow/openpal3/dlg_box.p7")
        .expect("openpal3/dlg_box.p7 must compile against radiance_scripting.ui");
}

#[test]
fn dialog_renders_both_portraits_with_positive_scaled_bounds_and_preserves_text_markup() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import radiance;
        import scripting_services;
        import shared.openpal3;
        import yaobow.openpal3.dlg_box;
        pub fn make(sprites: &scripting_services.ISpriteService) -> &openpal3.IPal3DialogRenderer {
            dlg_box.make_pal3_dialog_renderer(sprites)
        }
        pub fn frame(renderer: &openpal3.IPal3DialogRenderer, ui: &radiance.IUiHost,
            side: int, content: string) {
            if side < 0 { renderer.clear_avatar(); }
            else { renderer.set_avatar("JT", "JT00", side); }
            renderer.render_dialog(ui, content, 0.0);
        }
        pub fn advance(renderer: &openpal3.IPal3DialogRenderer) -> bool {
            renderer.advance_page()
        }
    "#,
    )
    .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    let ui_id = host.intern(ui);
    let sprites_id = host.intern(ComRc::<ISpriteService>::from_object(TestSprites));
    let ui = host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap();
    let sprites = host
        .foreign_box(
            "radiance_scripting.comdef.services.ISpriteService",
            sprites_id,
        )
        .unwrap();
    let renderer = host.call_returning_data("make", vec![sprites]).unwrap();

    for (width, height, scale, ox, oy) in [
        (800, 600, 1.0, 0.0, 0.0),
        (1600, 600, 1.0, 400.0, 0.0),
        (800, 1200, 1.0, 0.0, 300.0),
        (400, 300, 0.5, 0.0, 0.0),
    ] {
        recorder.set_display_size(width, height);
        for side in [-1, 0, 1] {
            recorder.calls.borrow_mut().clear();
            host.call_void(
                "frame",
                vec![
                    renderer.clone(),
                    ui.clone(),
                    Data::Int(side),
                    Data::string("river\\i黄\\r白\n\n末"),
                ],
            )
            .unwrap();
            let calls = recorder.calls.borrow();
            let portraits: Vec<_> = calls
                .iter()
                .filter(|c| {
                    matches!(
                        c,
                        UiCall::ImageRect {
                            texture_com_id: 77,
                            ..
                        }
                    )
                })
                .collect();
            assert_eq!(portraits.len(), usize::from(side >= 0));
            if let Some(UiCall::ImageRect {
                x0,
                y0,
                x1,
                y1,
                u0,
                u1,
                ..
            }) = portraits.first().copied()
            {
                assert_eq!(
                    (*x0, *y0, *x1, *y1),
                    (
                        ox + if side == 0 { 0.0 } else { 544.0 * scale },
                        oy + 88.0 * scale,
                        ox + if side == 0 {
                            256.0 * scale
                        } else {
                            800.0 * scale
                        },
                        oy + 600.0 * scale,
                    )
                );
                assert_eq!(
                    (*u0, *u1),
                    if side == 0 {
                        (0.75, 0.125)
                    } else {
                        (0.125, 0.75)
                    }
                );
            }
            assert!(calls.iter().any(|call| matches!(
                call,
                UiCall::ImageRect { texture_com_id: 88, x0, y0, x1, y1, .. }
                    if (*x0, *y0, *x1, *y1) == (
                        ox + if side == 1 { 18.0 } else { 760.0 } * scale,
                        oy + 552.0 * scale,
                        ox + if side == 1 { 50.0 } else { 792.0 } * scale,
                        oy + 584.0 * scale,
                    )
            )));
            let text: Vec<_> = calls
                .iter()
                .filter_map(|call| {
                    if let UiCall::TextAt {
                        x, y, size, b, s, ..
                    } = call
                    {
                        assert_eq!(*size, Some(20.0 * scale));
                        Some((*x, *y, *b, s.as_str()))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(text.iter().map(|t| t.3).collect::<String>(), "river黄白末");
            let left = match side {
                -1 => 36.0,
                0 => 276.0,
                _ => 54.0,
            };
            assert_eq!(
                (text[0].0, text[0].1),
                (ox + left * scale, oy + 474.0 * scale)
            );
            assert_eq!(text[5].2, 0.0);
            assert_eq!(text[6].2, 1.0);
            assert_eq!(text[1].0 - text[0].0, 10.0 * scale);
            assert_eq!(text[6].0 - text[5].0, 20.0 * scale);
            assert_eq!(text[7].1, oy + 518.0 * scale);
        }
    }
    recorder.set_display_size(800, 600);
    recorder.calls.borrow_mut().clear();
    host.call_void(
        "frame",
        vec![
            renderer.clone(),
            ui.clone(),
            Data::Int(0),
            Data::string("一".repeat(25)),
        ],
    )
    .unwrap();
    let calls = recorder.calls.borrow();
    let positions: Vec<_> = calls
        .iter()
        .filter_map(|c| {
            if let UiCall::TextAt { x, y, .. } = c {
                Some((*x, *y))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(positions[23], (736.0, 474.0));
    assert_eq!(positions[24], (276.0, 496.0));
    drop(calls);
    for (count, more) in [(120, true), (1, false)] {
        recorder.calls.borrow_mut().clear();
        host.call_void(
            "frame",
            vec![
                renderer.clone(),
                ui.clone(),
                Data::Int(0),
                Data::string(format!("\\i{}", "一".repeat(121))),
            ],
        )
        .unwrap();
        assert_eq!(
            recorder
                .calls
                .borrow()
                .iter()
                .filter(|c| matches!(c, UiCall::TextAt { .. }))
                .count(),
            count
        );
        assert_eq!(
            host.call_returning_data("advance", vec![renderer.clone()])
                .unwrap(),
            Data::Int(more as i64)
        );
    }
    recorder.calls.borrow_mut().clear();
    host.call_void(
        "frame",
        vec![renderer, ui, Data::Int(-1), Data::string("\\b大\\s小")],
    )
    .unwrap();
    let text: Vec<_> = recorder
        .calls
        .borrow()
        .iter()
        .filter_map(|call| {
            if let UiCall::TextAt { x, y, size, .. } = call {
                Some((*x, *y, *size))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(text, [(36.0, 500.0, Some(24.0)), (36.0, 526.0, Some(20.0))]);
}
