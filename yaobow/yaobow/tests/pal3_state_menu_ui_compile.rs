//! Compile and interaction regressions for the PAL3 status HUD and
//! character-status (状态) menu it delegates to. Loading `status_ui.p7`
//! transitively imports `state_menu.p7`, so this one load exercises both
//! against the production script VFS (so `import radiance_scripting.ui;`
//! and `import yaobow.openpal3.state_menu;` resolve).

use std::{cell::RefCell, rc::Rc, sync::Arc};

use crosscom::ComRc;
use p7::interpreter::context::Data;
use radiance_scripting::{
    ScriptHost,
    comdef::services::{IAtlasPage, ISprite, ISpriteImpl, ISpriteService, ISpriteServiceImpl},
    services::ui_host_recording::{RecordingUiHost, UiCall},
};
use shared::openpal3::{
    audio_mix::Pal3AudioMix,
    comdef::{IPal3UiAtlas, IPal3UiAtlasImpl},
    loaders::stats::StatsData,
    menu_state::Pal3MenuState,
    states::{persistent_state::PersistentState, stats::Resource},
};
use yaobow_lib::script_source::install_script_assets;

#[test]
fn pal3_status_ui_and_state_menu_compile() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source_from_path("/yaobow/openpal3/status_ui.p7")
        .expect("openpal3/status_ui.p7 (+ state_menu.p7) must compile");
}

#[test]
fn newly_opened_menu_leaves_one_clean_hud_frame_for_thumbnail_capture() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import radiance;
        import scripting_services;
        import shared.openpal3;
        import yaobow.openpal3.status_ui;
        pub fn make(sprites: &scripting_services.ISpriteService,
            atlas: &openpal3.IPal3UiAtlas) -> &openpal3.IPal3StatusRenderer {
            status_ui.make_pal3_status_renderer(sprites, atlas)
        }
        pub fn open(renderer: &openpal3.IPal3StatusRenderer, opened: bool) {
            renderer.set_menu_open(opened);
        }
        pub fn frame(renderer: &openpal3.IPal3StatusRenderer,
            ui: &radiance.IUiHost, game: &openpal3.IPal3MenuState) {
            renderer.render_status(ui, 0.0, game);
        }
    "#,
    )
    .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(800, 600);
    let ui_id = host.intern(ui);
    let sprites_id = host.intern(ComRc::<ISpriteService>::from_object(TestSprites));
    let atlas_id = host.intern(ComRc::<IPal3UiAtlas>::from_object(TestAtlas));
    let game_id = host.intern(Pal3MenuState::create(
        Rc::new(RefCell::new(PersistentState::new("capture-test".into()))),
        Some(Rc::new(menu_data())),
    ));
    let renderer = host
        .call_returning_data(
            "make",
            vec![
                host.foreign_box(
                    "radiance_scripting.comdef.services.ISpriteService",
                    sprites_id,
                )
                .unwrap(),
                host.foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
                    .unwrap(),
            ],
        )
        .unwrap();
    let frame = || {
        recorder.calls.borrow_mut().clear();
        host.call_void(
            "frame",
            vec![
                renderer.clone(),
                host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap(),
                host.foreign_box("shared.openpal3.comdef.IPal3MenuState", game_id)
                    .unwrap(),
            ],
        )
        .unwrap();
        recorder.calls.borrow().iter().any(|call| {
            matches!(call,
            UiCall::ImageRect {texture_com_id,..}
            if *texture_com_id == sprite_id("ui/GameMainUI/Communal/StpCtl/StpBarL.tga"))
        })
    };
    for _ in 0..2 {
        host.call_void("open", vec![renderer.clone(), Data::Int(1)])
            .unwrap();
        assert!(
            !frame(),
            "the first open frame must remain free of menu chrome"
        );
        assert!(
            frame(),
            "the menu appears after pre-update can capture the HUD"
        );
        host.call_void("open", vec![renderer.clone(), Data::Int(0)])
            .unwrap();
        assert!(!frame());
    }
}

#[test]
fn native_loading_screen_crops_progress_animates_and_resets_on_letterboxed_canvas() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import radiance;
        import scripting_services;
        import shared.openpal3;
        import yaobow.openpal3.status_ui;
        pub fn make(sprites: &scripting_services.ISpriteService,
            atlas: &openpal3.IPal3UiAtlas) -> &openpal3.IPal3StatusRenderer {
            status_ui.make_pal3_status_renderer(sprites, atlas)
        }
        pub fn paint(renderer: &openpal3.IPal3StatusRenderer,
            ui: &radiance.IUiHost, progress: float, dt: float) -> bool {
            renderer.render_loading(ui, progress, dt)
        }
        "#,
    )
    .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    let ui_id = host.intern(ui);
    let sprites_id = host.intern(ComRc::<ISpriteService>::from_object(TestSprites));
    let atlas_id = host.intern(ComRc::<IPal3UiAtlas>::from_object(TestAtlas));
    let renderer = host
        .call_returning_data(
            "make",
            vec![
                host.foreign_box(
                    "radiance_scripting.comdef.services.ISpriteService",
                    sprites_id,
                )
                .unwrap(),
                host.foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
                    .unwrap(),
            ],
        )
        .unwrap();
    let frame = |progress, dt| {
        recorder.calls.borrow_mut().clear();
        host.call_returning_data(
            "paint",
            vec![
                renderer.clone(),
                host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap(),
                Data::Float(progress),
                Data::Float(dt),
            ],
        )
        .unwrap()
    };
    for (width, height, ox, oy, scale) in [
        (800, 600, 0., 0., 1.),
        (1600, 600, 400., 0., 1.),
        (400, 300, 0., 0., 0.5),
        (800, 1200, 0., 300., 1.),
    ] {
        recorder.set_display_size(width, height);
        frame(0., 0.);
        assert!(recorder.calls.borrow().iter().any(|call| matches!(call,
            UiCall::FillRect {x0,y0,x1,y1,r,g,b,a}
            if (*x0,*y0,*x1,*y1,*r,*g,*b,*a)==(0.,0.,width as f32,height as f32,0.,0.,0.,1.))));
        let mut alternate_seen = false;
        for _ in 0..8 {
            assert_eq!(frame(1., 0.1), Data::Int(0));
            alternate_seen |= recorder.calls.borrow().iter().any(|call| {
                matches!(call,
                UiCall::ImageRect {texture_com_id,..}
                if *texture_com_id==sprite_id("/basedata/basedata/ui/Loading/loading_5.tga"))
            });
        }
        assert!(
            alternate_seen,
            "the native fairy must alternate between both sprites"
        );
        assert_eq!(frame(1., 4.25 / 216.), Data::Int(0));
        for (name, rect) in [
            (
                "/basedata/basedata/ui/Loading/loading_1.tga",
                [206., 520., 570., 548.],
            ),
            (
                "/basedata/basedata/ui/Loading/loading_2.tga",
                [210., 529., 387., 542.],
            ),
            (
                "/basedata/basedata/ui/Loading/loading_3.tga",
                [343., 483., 431., 547.],
            ),
        ] {
            assert!(recorder.calls.borrow().iter().any(|call| matches!(call,
                UiCall::ImageRect {texture_com_id,x0,y0,x1,y1,..}
                if *texture_com_id==sprite_id(name)
                && (*x0,*y0,*x1,*y1)==(ox+rect[0]*scale,oy+rect[1]*scale,ox+rect[2]*scale,oy+rect[3]*scale))),"{name}: {:?}",recorder.calls.borrow());
        }
        assert!(
            recorder.calls.borrow().iter().any(|call| matches!(call,
            UiCall::ImageRect {texture_com_id,u0,u1,..}
            if *texture_com_id==sprite_id("/basedata/basedata/ui/Loading/loading_2.tga")
            && (*u0,*u1)==(0.125,0.125+0.625*177.5/512.))),
            "crop the fill UV rather than squeezing the whole texture"
        );
        for _ in 0..9 {
            frame(1., 0.1);
        }
        assert_eq!(frame(1., 0.), Data::Int(1));
        frame(0., 0.);
        assert!(!recorder.calls.borrow().iter().any(|call| matches!(call,
            UiCall::ImageRect {texture_com_id,..} if *texture_com_id==sprite_id("/basedata/basedata/ui/Loading/loading_2.tga"))));
        assert_eq!(
            frame(1., 60.),
            Data::Int(1),
            "elapsed loading work must catch up without an artificial time cap"
        );
        frame(0., 0.);
        assert_eq!(frame(0.8, 2.), Data::Int(1));
        for _ in 0..3 {
            frame(0.8, 0.1);
            assert!(
                recorder.calls.borrow().iter().any(|call| matches!(call,
                UiCall::ImageRect {texture_com_id,x1,..}
                if *texture_com_id==sprite_id("/basedata/basedata/ui/Loading/loading_2.tga")
                && *x1==ox+(210.+283.)*scale)),
                "progress must stop at completed work, not keep filling with time"
            );
        }
        assert_eq!(frame(0.85, 0.), Data::Int(0));
        assert_eq!(frame(0.85, 0.1), Data::Int(1));
        assert_eq!(frame(1., 0.), Data::Int(0));
    }
}

#[test]
fn native_pickup_notice_centers_stacked_text_and_fades_without_a_panel() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import radiance;
        import scripting_services;
        import shared.openpal3;
        import yaobow.openpal3.status_ui;
        pub fn make(sprites: &scripting_services.ISpriteService,
            atlas: &openpal3.IPal3UiAtlas) -> &openpal3.IPal3StatusRenderer {
            status_ui.make_pal3_status_renderer(sprites, atlas)
        }
        pub fn paint(renderer: &openpal3.IPal3StatusRenderer,
            ui: &radiance.IUiHost, elapsed: float, row: int) {
            renderer.render_reward_notice(ui, "得到25文钱", elapsed, row);
        }
        "#,
    )
    .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(800, 600);
    let ui_id = host.intern(ui);
    let sprites_id = host.intern(ComRc::<ISpriteService>::from_object(TestSprites));
    let atlas_id = host.intern(ComRc::<IPal3UiAtlas>::from_object(TestAtlas));
    let renderer = host
        .call_returning_data(
            "make",
            vec![
                host.foreign_box(
                    "radiance_scripting.comdef.services.ISpriteService",
                    sprites_id,
                )
                .unwrap(),
                host.foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
                    .unwrap(),
            ],
        )
        .unwrap();
    for (elapsed, row, expected_alpha) in [(0., 0, 1.), (1.5, 1, 1.), (2.75, 2, 0.5), (4., 3, 0.)] {
        recorder.calls.borrow_mut().clear();
        host.call_void(
            "paint",
            vec![
                renderer.clone(),
                host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap(),
                Data::Float(elapsed),
                Data::Int(row),
            ],
        )
        .unwrap();
        let calls = recorder.calls.borrow();
        assert!(
            !calls
                .iter()
                .any(|call| matches!(call, UiCall::ImageRect { .. }))
        );
        assert!(calls.iter().any(|call| matches!(call,
            UiCall::TextAt { x, y, size: Some(size), r, g, b, a, s }
            if (*x, *y, *size) == (350., 40. + row as f32 * 24., 20.)
                && (*r, *g, *b) == (1., 1., 1.)
                && (*a - expected_alpha).abs() < 0.0001 && s == "得到25文钱")));
    }
}

#[test]
fn native_mechanism_notice_uses_shipped_note_nine_slice() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import radiance;
        import scripting_services;
        import shared.openpal3;
        import yaobow.openpal3.status_ui;
        pub fn make(sprites: &scripting_services.ISpriteService,
            atlas: &openpal3.IPal3UiAtlas) -> &openpal3.IPal3StatusRenderer {
            status_ui.make_pal3_status_renderer(sprites, atlas)
        }
        pub fn paint(renderer: &openpal3.IPal3StatusRenderer,
            ui: &radiance.IUiHost, text: string) {
            renderer.render_notice(ui, text);
        }
        "#,
    )
    .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(800, 600);
    let ui_id = host.intern(ui);
    let sprites_id = host.intern(ComRc::<ISpriteService>::from_object(TestSprites));
    let atlas_id = host.intern(ComRc::<IPal3UiAtlas>::from_object(TestAtlas));
    let renderer = host
        .call_returning_data(
            "make",
            vec![
                host.foreign_box(
                    "radiance_scripting.comdef.services.ISpriteService",
                    sprites_id,
                )
                .unwrap(),
                host.foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
                    .unwrap(),
            ],
        )
        .unwrap();
    host.call_void(
        "paint",
        vec![
            renderer.clone(),
            host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap(),
            Data::string("重量不够"),
        ],
    )
    .unwrap();

    let calls = recorder.calls.borrow();
    assert_eq!(
        calls
            .iter()
            .filter(|call| matches!(call, UiCall::ImageRect { .. }))
            .count(),
        9
    );
    assert!(calls.iter().any(|call| matches!(call,
        UiCall::ImageRect { x0, y0, x1, y1, .. }
            if (*x0, *y0, *x1, *y1) == (263.0, 10.0, 295.0, 42.0))));
    assert!(calls.iter().any(|call| matches!(call,
        UiCall::TextAt { x, y, size: Some(size), r, g, b, a, s }
            if (*x, *y, *size) == (360.0, 38.5, 20.0)
            && (*r, *g, *b, *a) == (250.0 / 255.0, 1.0, 1.0, 1.0)
            && s == "重量不够")));
    drop(calls);
    recorder.calls.borrow_mut().clear();
    host.call_void(
        "paint",
        vec![
            renderer,
            host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap(),
            Data::string(
                "得到布袍\n得到铁剑\n得到护腕\n得到草鞋\n得到止血草\n得到蜂王蜜\n经验增加55",
            ),
        ],
    )
    .unwrap();
    let calls = recorder.calls.borrow();
    let bottom = calls
        .iter()
        .filter_map(|call| match call {
            UiCall::ImageRect { y1, .. } => Some(*y1),
            _ => None,
        })
        .fold(0., f32::max);
    assert!(bottom > 87., "multi-item rewards must grow the note panel");
    let text = calls
        .iter()
        .filter_map(|call| match call {
            UiCall::TextAt {
                y,
                size: Some(size),
                ..
            } => Some((*y, *size)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        text.len(),
        7,
        "every granted item and EXP notice must be drawn"
    );
    assert!(text.iter().all(|(y, size)| *y >= 10. && y + size <= bottom));
}

#[test]
fn system_audio_uses_native_geometry_and_captured_independent_sliders() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import radiance;
        import shared.openpal3;
        import yaobow.openpal3.save_load;
        pub fn make() -> &save_load.AudioControls { save_load.make_audio_controls() }
        pub fn frame(audio: &save_load.AudioControls, ui: &radiance.IUiHost,
            atlas: &openpal3.IPal3UiAtlas, game: &openpal3.IPal3MenuState,
            x: float, y: float, press: bool, release: bool, down: bool, modal: bool) -> bool {
            audio.paint(ui, atlas, game, x, y, press, release, down, modal);
            true
        }
        "#,
    )
    .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    let service = Pal3MenuState::create_with_audio(
        Rc::new(RefCell::new(PersistentState::new("audio-ui-test".into()))),
        Some(Rc::new(menu_data())),
        Arc::new(Pal3AudioMix::new(0.25, 0.75)),
    );
    assert!(!service.save_audio_settings());
    let ui_id = host.intern(ui);
    let atlas_id = host.intern(ComRc::<IPal3UiAtlas>::from_object(TestAtlas));
    let game_id = host.intern(service.clone());
    let ui = host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap();
    let atlas = host
        .foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
        .unwrap();
    let game = host
        .foreign_box("shared.openpal3.comdef.IPal3MenuState", game_id)
        .unwrap();
    let audio = host.call_returning_data("make", vec![]).unwrap();
    let frame = |x: f64, y: f64, press: bool, release: bool, down: bool, modal: bool| {
        recorder.calls.borrow_mut().clear();
        host.call_returning_data(
            "frame",
            vec![
                audio.clone(),
                ui.clone(),
                atlas.clone(),
                game.clone(),
                Data::Float(x),
                Data::Float(y),
                Data::Int(press as i64),
                Data::Int(release as i64),
                Data::Int(down as i64),
                Data::Int(modal as i64),
            ],
        )
        .unwrap();
    };
    for (width, height, ox, scale) in [
        (800, 600, 0.0, 1.0),
        (1600, 600, 400.0, 1.0),
        (1600, 1200, 0.0, 2.0),
    ] {
        recorder.set_display_size(width, height);
        frame(0.0, 0.0, false, false, false, false);
        for (name, x, y, w, h) in [
            (
                "ui/GameMainUI/System/MusicBar.tga",
                698.0,
                56.0,
                18.0,
                308.0,
            ),
            (
                "ui/GameMainUI/System/MusicEffect.tga",
                744.0,
                136.0,
                18.0,
                308.0,
            ),
            (
                "ui/GameMainUI/System/MusicText.tga",
                684.0,
                368.0,
                55.0,
                23.0,
            ),
            (
                "ui/GameMainUI/System/MusicEffectText.tga",
                726.0,
                105.0,
                55.0,
                23.0,
            ),
            (
                "ui/GameMainUI/Communal/Scroll/scrollbar0.tga",
                695.0,
                129.25,
                26.0,
                30.0,
            ),
            (
                "ui/GameMainUI/Communal/Scroll/scrollbar0.tga",
                740.0,
                204.25,
                26.0,
                30.0,
            ),
        ] {
            assert!(
                recorder.calls.borrow().iter().any(|call| matches!(call,
                UiCall::ImageRect { texture_com_id, x0, y0, x1, y1, .. }
                    if *texture_com_id == sprite_id(name)
                    && (*x0, *y0, *x1, *y1) == ((ox + x * scale) as f32, (y * scale) as f32,
                        (ox + (x + w) * scale) as f32, ((y + h) * scale) as f32))),
                "{name}"
            );
        }
    }
    recorder.set_display_size(1600, 600);
    frame(1108.0, 134.25, true, false, true, false);
    assert_eq!(
        service.music_volume(),
        0.25,
        "grabbing the thumb must not jump"
    );
    frame(1108.0, 134.25, false, true, false, false);
    assert_eq!(
        service.music_volume(),
        0.25,
        "stationary clicks must not change the gain"
    );
    frame(1108.0, 134.25, true, false, true, false);
    frame(1108.0, 270.75, false, false, true, false);
    assert_eq!(service.music_volume(), 0.75);
    assert_eq!(
        service.sound_volume(),
        0.75,
        "music must not change effects"
    );
    frame(-100.0, 1000.0, false, false, true, false);
    assert_eq!(
        service.music_volume(),
        1.0,
        "capture continues outside the viewport"
    );
    frame(-100.0, 1000.0, false, true, false, false);
    assert!(
        recorder.calls.borrow().iter().any(|call| matches!(call,
        UiCall::TextAt { s, .. } if s == "音量设置保存失败")),
        "{:?}",
        recorder.calls.borrow()
    );

    frame(1150.0, 209.25, true, false, true, false);
    assert_eq!(service.sound_volume(), 0.75);
    frame(1150.0, 1000.0, false, false, true, false);
    assert_eq!(
        service.sound_volume(),
        0.0,
        "effects increase upward, unlike music"
    );
    frame(1150.0, 1000.0, false, true, false, false);
    assert_eq!(service.music_volume(), 1.0);
    frame(1150.0, 151.0, true, false, true, true);
    assert_eq!(
        service.sound_volume(),
        0.0,
        "confirmation dialogs disable sliders"
    );
    frame(400.0, 10.0, true, false, true, false);
    frame(1150.0, 151.0, false, false, true, false);
    assert_eq!(
        service.sound_volume(),
        0.0,
        "a drag cannot begin outside the control"
    );
}

#[test]
fn system_audio_persists_on_release_not_during_drag() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import radiance;
        import shared.openpal3;
        import yaobow.openpal3.save_load;
        pub fn make() -> &save_load.AudioControls { save_load.make_audio_controls() }
        pub fn frame(audio: &save_load.AudioControls, ui: &radiance.IUiHost,
            atlas: &openpal3.IPal3UiAtlas, game: &openpal3.IPal3MenuState, finish: bool) -> bool {
            audio.paint(ui, atlas, game, 708.0, 200.0, !finish, finish, !finish, false);
            true
        }
        "#,
    )
    .unwrap();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-data")
        .join(format!("pal3-audio-ui-{}.toml", std::process::id()));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    assert!(!path.exists());
    let service = Pal3MenuState::create_with_audio_config_path(
        Rc::new(RefCell::new(PersistentState::new("audio-ui-test".into()))),
        Some(Rc::new(menu_data())),
        Arc::new(Pal3AudioMix::new(0.25, 0.75)),
        path.clone(),
    );
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(800, 600);
    let ui_id = host.intern(ui);
    let atlas_id = host.intern(ComRc::<IPal3UiAtlas>::from_object(TestAtlas));
    let game_id = host.intern(service.clone());
    let inputs = vec![
        host.call_returning_data("make", vec![]).unwrap(),
        host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap(),
        host.foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
            .unwrap(),
        host.foreign_box("shared.openpal3.comdef.IPal3MenuState", game_id)
            .unwrap(),
    ];
    let mut drag = inputs.clone();
    drag.push(Data::Int(0));
    host.call_returning_data("frame", drag).unwrap();
    assert_ne!(service.music_volume(), 0.25);
    assert_eq!(service.sound_volume(), 0.75);
    assert!(!path.exists(), "drag frames must not write configuration");
    let mut release = inputs;
    release.push(Data::Int(1));
    host.call_returning_data("frame", release).unwrap();
    assert!(path.exists(), "releasing the slider persists the live mix");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn system_menu_keeps_slider_capture_and_requires_confirmation_to_return_to_menu() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import radiance;
        import shared.openpal3;
        import yaobow.openpal3.state_menu;
        pub fn make() -> &state_menu.MenuState { state_menu.make_menu() }
        pub fn frame(menu: &state_menu.MenuState, ui: &radiance.IUiHost,
            atlas: &openpal3.IPal3UiAtlas, game: &openpal3.IPal3MenuState,
            x: float, y: float, press: bool, release: bool, down: bool) -> bool {
            menu.paint(ui, atlas, game, x, y, press, release, down)
        }
        "#,
    )
    .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(1600, 600);
    let service = Pal3MenuState::create_with_audio(
        Rc::new(RefCell::new(PersistentState::new(
            "system-menu-test".into(),
        ))),
        Some(Rc::new(menu_data())),
        Arc::new(Pal3AudioMix::new(0.25, 0.75)),
    );
    let ui_id = host.intern(ui);
    let atlas_id = host.intern(ComRc::<IPal3UiAtlas>::from_object(TestAtlas));
    let game_id = host.intern(service.clone());
    let inputs = vec![
        host.call_returning_data("make", vec![]).unwrap(),
        host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap(),
        host.foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
            .unwrap(),
        host.foreign_box("shared.openpal3.comdef.IPal3MenuState", game_id)
            .unwrap(),
    ];
    let frame = |x, y, press: bool, release: bool, down: bool| {
        recorder.calls.borrow_mut().clear();
        let mut args = inputs.clone();
        args.extend([
            Data::Float(x),
            Data::Float(y),
            Data::Int(press as i64),
            Data::Int(release as i64),
            Data::Int(down as i64),
        ]);
        host.call_returning_data("frame", args).unwrap()
    };
    let click = |x, y| {
        frame(x, y, true, false, true);
        frame(x, y, false, true, false);
    };
    let requested_exit = || {
        service
            .inner::<Pal3MenuState>()
            .take_requested_return_to_menu()
    };
    click(1100.0, 570.0);
    frame(1108.0, 134.25, true, false, true);
    frame(1108.0, 270.75, false, false, true);
    assert_eq!(
        service.music_volume(),
        0.75,
        "the full menu must retain capture between frames"
    );
    frame(-100.0, 1000.0, false, true, false);
    assert_eq!(service.music_volume(), 1.0);
    frame(1150.0, 209.25, true, false, true);
    frame(1150.0, 345.75, false, false, true);
    assert_eq!(service.sound_volume(), 0.25);
    frame(1150.0, 345.75, false, true, false);
    assert_eq!(service.music_volume(), 1.0);
    frame(0.0, 0.0, false, false, false);
    assert!(recorder.calls.borrow().iter().any(|call| matches!(call,
        UiCall::ImageRect { texture_com_id, x0, y0, x1, y1, .. }
            if *texture_com_id == sprite_id("ui/GameMainUI/System/ExitGame0.tga")
            && (*x0, *y0, *x1, *y1) == (1096.0, 456.0, 1169.0, 531.0))));
    click(1130.0, 490.0);
    assert!(
        !requested_exit(),
        "the exit button must only open a confirmation"
    );
    assert!(recorder.calls.borrow().iter().any(|call| matches!(call,
        UiCall::TextAt { s, .. } if s.contains("未保存的进度将丢失"))));
    for (name, expected) in [
        ("DlgBackground.tga", (650.0, 200.0, 958.0, 309.0)),
        ("Yes0.tga", (676.0, 277.0, 766.0, 303.0)),
        ("No0.tga", (842.0, 277.0, 932.0, 303.0)),
    ] {
        assert!(
            recorder.calls.borrow().iter().any(|call| matches!(call,
            UiCall::ImageRect { texture_com_id, x0, y0, x1, y1, .. }
                if *texture_com_id == sprite_id(&format!("ui/GameMainUI/MUIDialog/Normal/{name}"))
                && (*x0, *y0, *x1, *y1) == expected)),
            "{name}"
        );
    }
    assert!(
        !recorder.calls.borrow().iter().any(|call| matches!(call,
        UiCall::TextAt { s, .. } if s == "确定" || s == "取消")),
        "the original button sprites already contain their labels"
    );
    frame(721.0, 290.0, false, false, false);
    assert!(recorder.calls.borrow().iter().any(|call| matches!(call,
        UiCall::ImageRect { texture_com_id, .. }
            if *texture_com_id == sprite_id("ui/GameMainUI/MUIDialog/Normal/Yes1.tga"))));
    assert!(
        !recorder.calls.borrow().iter().any(|call| matches!(call,
        UiCall::ImageRect { texture_com_id, .. }
            if *texture_com_id == sprite_id("ui/GameMainUI/System/Note1.tga")
                || *texture_com_id == sprite_id("ui/GameMainUI/System/Note2.tga"))),
        "a modal must not hover or press the save row underneath"
    );
    recorder.set_mouse_wheel(-1.0);
    frame(721.0, 290.0, false, false, false);
    recorder.set_mouse_wheel(0.0);
    assert!(
        recorder.calls.borrow().iter().any(|call| matches!(call,
        UiCall::TextAt { s, .. } if s == "1 - 5 / 36")),
        "scrolling over the confirmation must not page the underlying browser"
    );
    frame(580.0, 570.0, true, false, true);
    assert!(
        !recorder.calls.borrow().iter().any(|call| matches!(call,
        UiCall::ImageRect { texture_com_id, .. }
            if *texture_com_id == sprite_id("ui/GameMainUI/Communal/StpCtl/Prop1.tga")
                || *texture_com_id == sprite_id("ui/GameMainUI/Communal/StpCtl/Prop2.tga"))),
        "the confirmation blocks toolbar hover and press feedback"
    );
    frame(580.0, 570.0, false, true, false);
    click(1150.0, 151.0);
    assert_eq!(
        service.sound_volume(),
        0.25,
        "the confirmation blocks audio input"
    );
    frame(721.0, 290.0, true, false, true);
    assert!(recorder.calls.borrow().iter().any(|call| matches!(call,
        UiCall::ImageRect { texture_com_id, .. }
            if *texture_com_id == sprite_id("ui/GameMainUI/MUIDialog/Normal/Yes2.tga"))));
    frame(800.0, 400.0, false, true, false);
    assert!(!requested_exit(), "dragging away cancels the confirm click");
    click(887.0, 290.0);
    assert!(!requested_exit(), "cancel must keep the adventure running");
    click(1130.0, 490.0);
    frame(721.0, 290.0, true, false, true);
    assert!(!requested_exit(), "press alone must not return to the menu");
    assert_eq!(frame(721.0, 290.0, false, true, false), Data::Int(1));
    assert!(
        requested_exit(),
        "confirmed exit queues a host-side menu transition"
    );
    assert!(!requested_exit(), "the transition is drained once");
}

mod openpal3 {
    pub use shared::openpal3::comdef;
}

mod comdef {
    pub use radiance_scripting::comdef::*;
}

struct TestSprite(i32);
radiance_scripting::ComObject_Sprite!(crate::TestSprite);
impl ISpriteImpl for TestSprite {
    fn com_id(&self) -> i32 {
        self.0
    }
    fn width(&self) -> i32 {
        let base = "/basedata/basedata/ui/Loading/loading_";
        if [1, 2]
            .iter()
            .any(|i| self.0 == sprite_id(&format!("{base}{i}.tga")))
        {
            512
        } else if [3, 5]
            .iter()
            .any(|i| self.0 == sprite_id(&format!("{base}{i}.tga")))
        {
            128
        } else {
            256
        }
    }
    fn height(&self) -> i32 {
        let base = "/basedata/basedata/ui/Loading/loading_";
        if self.0 == sprite_id(&format!("{base}2.tga")) {
            8
        } else if [3, 5]
            .iter()
            .any(|i| self.0 == sprite_id(&format!("{base}{i}.tga")))
        {
            64
        } else {
            32
        }
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
        Some(ComRc::from_object(TestSprite(sprite_id(path))))
    }

    fn load_atlas_page(&self, _: &str) -> Option<ComRc<IAtlasPage>> {
        None
    }
}

fn sprite_id(name: &str) -> i32 {
    (name.bytes().fold(17u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as u32)
    }) & 0x7fffffff) as i32
}

struct TestAtlas;
shared::ComObject_Pal3UiAtlas!(crate::TestAtlas);

impl IPal3UiAtlasImpl for TestAtlas {
    fn has_sprite(&self, _: &str) -> bool {
        true
    }
    fn sprite(&self, name: &str) -> Option<ComRc<ISprite>> {
        Some(ComRc::from_object(TestSprite(sprite_id(name))))
    }
    fn native_width(&self) -> i32 {
        800
    }
    fn native_height(&self) -> i32 {
        600
    }
}

fn menu_data() -> StatsData {
    menu_data_with_items(&[
        (42, "Medicine", 1),
        (43, "Second", 1),
        (82201, "Weapon", 11),
    ])
}

fn menu_data_with_items(items: &[(u32, &str, u8)]) -> StatsData {
    menu_data_with_scopes(items, &[])
}

fn menu_data_with_scopes(items: &[(u32, &str, u8)], party_items: &[u32]) -> StatsData {
    let mut database = vec![0u8; 0xa0 + items.len() * 800];
    let magic = b"__PAL(Softstar_Ltd: Cool_J)_GameDataBase_OutFile__";
    database[..magic.len()].copy_from_slice(magic);
    database[0x90..0x94].copy_from_slice(&0xa0u32.to_le_bytes());
    database[0x94..0x98].copy_from_slice(&(items.len() as u32).to_le_bytes());
    for (index, (id, name, category)) in items.iter().enumerate() {
        let record = &mut database[0xa0 + index * 800..0xa0 + (index + 1) * 800];
        record[..4].copy_from_slice(&id.to_le_bytes());
        record[4..4 + name.len()].copy_from_slice(name.as_bytes());
        record[0x60..0x6c].copy_from_slice(b"$Description");
        record[0x260..0x264].copy_from_slice(&17i32.to_le_bytes());
        record[0x264] = *category;
        if *category == 1 {
            record[0x275] = if party_items.contains(id) { 2 } else { 1 };
            record[0x276] = 3;
            record[0x284..0x286].copy_from_slice(&20i16.to_le_bytes());
        }
        record[0x267] = 1;
        if *id == 9001 {
            record[0x28a..0x28c].copy_from_slice(&9i16.to_le_bytes());
        }
    }
    let combat = format!(
        "[Level]\n{}",
        (1..=99)
            .map(|level| format!("{level}={}\n", (level - 1) * 100))
            .collect::<String>()
    );
    StatsData::load(&database, &combat).unwrap()
}

#[test]
fn explicit_menu_actions_target_items_and_transfer_equipment_only_on_release() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import radiance;
        import shared.openpal3;
        import yaobow.openpal3.state_menu;
        pub fn make() -> &state_menu.MenuState { state_menu.make_menu() }
        pub fn frame(menu: &state_menu.MenuState, ui: &radiance.IUiHost,
            atlas: &openpal3.IPal3UiAtlas, game: &openpal3.IPal3MenuState,
            x: float, y: float, press: bool, release: bool, down: bool) -> bool {
            menu.paint(ui, atlas, game, x, y, press, release, down)
        }
    "#,
    )
    .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(800, 600);
    let data = Rc::new(menu_data_with_items(&[
        (42, "Medicine", 1),
        (43, "Other", 1),
        (82201, "Old weapon", 11),
        (9001, "New weapon", 11),
    ]));
    let state = Rc::new(RefCell::new(PersistentState::new("test".into())));
    {
        let mut state = state.borrow_mut();
        state.party_mut().set_in_team(1, true);
        state.stats_mut().set_resource(0, Resource::Hp, 12, &data);
        state.add_item(42, 2, &data);
        state.add_item(43, 1, &data);
        state.add_item(9001, 1, &data);
    }
    let service = Pal3MenuState::create(state.clone(), Some(data.clone()));
    let ui_id = host.intern(ui);
    let atlas_id = host.intern(ComRc::<IPal3UiAtlas>::from_object(TestAtlas));
    let game_id = host.intern(service);
    let ui = host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap();
    let atlas = host
        .foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
        .unwrap();
    let game = host
        .foreign_box("shared.openpal3.comdef.IPal3MenuState", game_id)
        .unwrap();
    let menu = host.call_returning_data("make", vec![]).unwrap();
    let frame = |x: f64, y: f64, press: bool, release: bool, down: bool| {
        host.call_returning_data(
            "frame",
            vec![
                menu.clone(),
                ui.clone(),
                atlas.clone(),
                game.clone(),
                Data::Float(x),
                Data::Float(y),
                Data::Int(press as i64),
                Data::Int(release as i64),
                Data::Int(down as i64),
            ],
        )
        .unwrap();
    };
    let click = |x, y| {
        frame(x, y, true, false, true);
        frame(x, y, false, true, false);
    };
    click(180.0, 570.0);
    frame(0.0, 0.0, false, false, false);
    click(300.0, 205.0);
    assert_eq!(
        state.borrow().stats().character(0).unwrap().current().hp,
        12
    );
    frame(300.0, 505.0, true, false, true);
    assert_eq!(state.borrow().inventory().quantity(42), 2);
    frame(300.0, 505.0, false, true, false);
    assert_eq!(
        state.borrow().inventory().quantity(42),
        2,
        "arming requires an explicit target"
    );
    click(740.0, 210.0);
    assert_eq!(
        state.borrow().inventory().quantity(42),
        2,
        "full target must not consume"
    );
    click(740.0, 100.0);
    assert_eq!(
        state.borrow().stats().character(0).unwrap().current().hp,
        32
    );
    assert_eq!(state.borrow().inventory().quantity(42), 1);
    click(740.0, 100.0);
    assert_eq!(
        state.borrow().inventory().quantity(42),
        1,
        "one action cannot repeat"
    );

    // A different item becoming selected between press/release cancels the action.
    frame(300.0, 505.0, true, false, true);
    state.borrow_mut().remove_item(42, &data);
    frame(300.0, 505.0, false, true, false);
    click(740.0, 100.0);
    assert_eq!(state.borrow().inventory().quantity(43), 1);
    assert_eq!(
        state.borrow().stats().character(0).unwrap().current().hp,
        32
    );

    click(740.0, 210.0);
    click(268.0, 570.0);
    frame(0.0, 0.0, false, false, false);
    click(300.0, 205.0);
    assert_eq!(
        state.borrow().stats().character(1).unwrap().equipment()[5],
        Some(82201)
    );
    frame(300.0, 505.0, true, false, true);
    assert_eq!(state.borrow().inventory().quantity(9001), 1);
    frame(300.0, 505.0, false, true, false);
    assert_eq!(
        state.borrow().stats().character(1).unwrap().equipment()[5],
        Some(9001)
    );
    assert_eq!(
        state.borrow().stats().attributes(1, &data).unwrap().attack,
        37
    );
    assert!(!state.borrow().inventory().contains(9001));
    assert_eq!(state.borrow().inventory().quantity(82201), 1);
    click(300.0, 505.0);
    assert_eq!(
        state.borrow().stats().character(1).unwrap().equipment()[5],
        None
    );
    assert_eq!(state.borrow().inventory().quantity(9001), 1);
    assert_eq!(state.borrow().inventory().quantity(82201), 1);
}

#[test]
fn native_inventory_click_clock_matches_polled_release_boundaries() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import yaobow.openpal3.state_menu;
        pub fn make() -> &state_menu.MenuState { state_menu.make_menu() }
        pub fn tick(menu: &state_menu.MenuState, now: int, down: bool) -> bool {
            menu.tick(now, down)
        }
    "#,
    )
    .unwrap();
    for (idle, expected) in [(99, true), (100, true), (101, false)] {
        for hold in [10, 1000] {
            let menu = host.call_returning_data("make", vec![]).unwrap();
            let tick = |now, down| {
                host.call_returning_data(
                    "tick",
                    vec![menu.clone(), Data::Int(now), Data::Int(down as i64)],
                )
                .unwrap()
            };
            assert_eq!(tick(1000, false), Data::Int(0));
            assert_eq!(tick(1010, true), Data::Int(0));
            tick(1010 + hold, false);
            tick(1010 + hold + idle, false);
            assert_eq!(tick(1011 + hold + idle, true), Data::Int(expected as i64));
            assert_eq!(
                tick(1020 + hold + idle, true),
                Data::Int(0),
                "holding never repeats a click"
            );
        }
    }
    let menu = host.call_returning_data("make", vec![]).unwrap();
    let tick = |now, down| {
        host.call_returning_data(
            "tick",
            vec![menu.clone(), Data::Int(now), Data::Int(down as i64)],
        )
        .unwrap()
    };
    tick(2147483600, false);
    tick(2147483610, true);
    tick(2147483620, false);
    assert_eq!(
        tick(20, true),
        Data::Int(1),
        "the clock wraps without losing the click cycle"
    );
    tick(30, false);
    assert_eq!(
        tick(500, true),
        Data::Int(1),
        "a third click remains second-state; expiry is polled only while released"
    );
}

#[test]
fn native_inventory_second_click_targets_reuses_and_cancels_without_consuming_invalid_actions() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import radiance;
        import shared.openpal3;
        import yaobow.openpal3.state_menu;
        pub fn make() -> &state_menu.MenuState { state_menu.make_menu() }
        pub fn second(menu: &state_menu.MenuState) -> bool {
            menu.tick(1000, true); menu.tick(1010, false); menu.tick(1020, true)
        }
        pub fn clock(menu: &state_menu.MenuState, now: int) -> bool { menu.tick(now, false) }
        pub fn frame(menu: &state_menu.MenuState, ui: &radiance.IUiHost,
            atlas: &openpal3.IPal3UiAtlas, game: &openpal3.IPal3MenuState,
            x: float, y: float, press: bool, release: bool, down: bool) -> bool {
            menu.paint(ui, atlas, game, x, y, press, release, down)
        }
    "#,
    )
    .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(1600, 900);
    let data = Rc::new(menu_data_with_scopes(
        &[
            (42, "Medicine", 1),
            (43, "Other", 1),
            (44, "Party medicine", 1),
        ],
        &[44],
    ));
    let state = Rc::new(RefCell::new(PersistentState::new("test".into())));
    {
        let mut state = state.borrow_mut();
        state.party_mut().set_in_team(1, true);
        state.stats_mut().set_resource(0, Resource::Hp, 12, &data);
        state.add_item(42, 5, &data);
        state.add_item(43, 2, &data);
    }
    let ui_id = host.intern(ui);
    let atlas_id = host.intern(ComRc::<IPal3UiAtlas>::from_object(TestAtlas));
    let service = Pal3MenuState::create(state.clone(), Some(data.clone()));
    let game_id = host.intern(service.clone());
    let ui = host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap();
    let atlas = host
        .foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
        .unwrap();
    let game = host
        .foreign_box("shared.openpal3.comdef.IPal3MenuState", game_id)
        .unwrap();
    let menu = host.call_returning_data("make", vec![]).unwrap();
    let frame = |x: f64, y: f64, press: bool, release: bool, down: bool| {
        recorder.calls.borrow_mut().clear();
        host.call_returning_data(
            "frame",
            vec![
                menu.clone(),
                ui.clone(),
                atlas.clone(),
                game.clone(),
                Data::Float(200. + x * 1.5),
                Data::Float(y * 1.5),
                Data::Int(press as i64),
                Data::Int(release as i64),
                Data::Int(down as i64),
            ],
        )
        .unwrap();
    };
    let click = |x, y| {
        frame(x, y, true, false, true);
        frame(x, y, false, true, false);
    };
    let second_click = |x, y| {
        host.call_returning_data("second", vec![menu.clone()])
            .unwrap();
        frame(x, y, true, false, true);
        frame(x, y, false, true, false);
    };
    let armed = || {
        recorder.calls.borrow().iter().any(|call| {
            matches!(
                call, UiCall::TextAt { s, .. } if s == "双击队员使用（右键取消）"
            )
        })
    };
    let hp = || state.borrow().stats().character(0).unwrap().current().hp;
    click(180., 570.);
    click(300., 208.);
    assert!(!armed(), "a single click selects without arming");
    second_click(300., 208.);
    assert!(armed());
    assert_eq!(
        service.inner::<Pal3MenuState>().take_menu_sounds(),
        ["wa002"]
    );
    assert!(
        recorder.calls.borrow().iter().any(|call| matches!(
            call, UiCall::ImageRect { texture_com_id, x0, y0, x1, y1, .. }
            if *texture_com_id == sprite_id("ui/GameMainUI/Communal/NeedArrow.tga")
                && (*x0, *y0, *x1, *y1) == (1274., 244.5, 1311.5, 285.)
        )),
        "the native arrow uses the letterboxed portrait geometry"
    );
    for (time, y) in [
        (1036, 6),
        (1037, 4),
        (2000, 4),
        (2037, 2),
        (2038, 2),
        (2075, 0),
        (2076, 0),
        (2113, 2),
    ] {
        host.call_returning_data("clock", vec![menu.clone(), Data::Int(time)])
            .unwrap();
        frame(0., 0., false, false, false);
        assert!(
            recorder.calls.borrow().iter().any(|call| matches!(
                call, UiCall::ImageRect { texture_com_id, y0, .. }
                if *texture_com_id == sprite_id("ui/GameMainUI/Communal/NeedArrow.tga")
                    && *y0 == (157. + y as f32) * 1.5
            )),
            "the arrow takes one two-pixel step only after >36ms, without catch-up"
        );
    }
    assert_eq!(state.borrow().inventory().quantity(42), 5);
    click(300., 234.);
    click(740., 100.);
    assert_eq!(hp(), 12, "single target clicks do not apply native actions");
    second_click(740., 210.);
    assert_eq!(
        service.inner::<Pal3MenuState>().take_menu_sounds(),
        ["wa004"]
    );
    assert!(
        !recorder
            .calls
            .borrow()
            .iter()
            .any(|call| matches!(call, UiCall::AdditiveImage { .. }))
    );
    assert_eq!(
        state.borrow().inventory().quantity(42),
        5,
        "a full target does not consume"
    );
    assert!(armed(), "another injured member remains eligible");
    second_click(740., 100.);
    assert!(
        service
            .inner::<Pal3MenuState>()
            .take_menu_sounds()
            .is_empty(),
        "normal successful use adds no invented sound"
    );
    assert_eq!(
        recorder
            .calls
            .borrow()
            .iter()
            .filter(|call| matches!(call,
                UiCall::AdditiveImage { texture_com_id, rect, uv, color }
                if *texture_com_id == sprite_id("ui/GameMainUI/Par_Effect/spark.tga")
                    && (rect[2] - rect[0], rect[3] - rect[1]) == (48., 48.)
                    && *uv == [0.125,0.25,0.75,0.875]
                    && color[0] >= 168. / 255. && color[1] >= 168. / 255. && color[2] < 38. / 255.
            ))
            .count(),
        24,
        "twelve rising/falling pairs use the shipped spark and HP tint"
    );
    assert_eq!(hp(), 32);
    assert_eq!(
        state.borrow().inventory().quantity(42),
        4,
        "locked lists retain the original item"
    );
    assert_eq!(state.borrow().inventory().quantity(43), 2);
    second_click(740., 100.);
    assert_eq!(hp(), 52);
    assert_eq!(state.borrow().inventory().quantity(42), 3);
    assert!(armed(), "native targeting persists across successful uses");
    recorder.mouse_clicked_buttons.borrow_mut()[1] = true;
    frame(300., 208., false, false, false);
    recorder.mouse_clicked_buttons.borrow_mut()[1] = false;
    assert!(
        service
            .inner::<Pal3MenuState>()
            .take_menu_sounds()
            .is_empty(),
        "right-click cancellation is silent"
    );
    assert!(!armed());
    second_click(740., 100.);
    assert_eq!(hp(), 52, "right-click cancellation prevents subsequent use");
    second_click(300., 208.);
    assert!(armed());
    for _ in 0..3 {
        state.borrow_mut().remove_item(42, &data);
    }
    second_click(740., 100.);
    assert_eq!(
        hp(),
        52,
        "inventory changes invalidate the pinned target item"
    );
    assert_eq!(state.borrow().inventory().quantity(43), 2);
    assert!(!armed());
    state
        .borrow_mut()
        .stats_mut()
        .set_resource(0, Resource::Hp, 9999, &data);
    second_click(300., 208.);
    assert!(!armed(), "no eligible member means no targeting mode");
    let maximum = hp();
    state
        .borrow_mut()
        .stats_mut()
        .set_resource(0, Resource::Hp, maximum - 1, &data);
    second_click(300., 208.);
    second_click(740., 100.);
    assert_eq!(hp(), maximum);
    assert_eq!(state.borrow().inventory().quantity(43), 1);
    assert!(
        !armed(),
        "targeting ends when nobody benefits, even with stock left"
    );
    state
        .borrow_mut()
        .stats_mut()
        .set_resource(0, Resource::Hp, 12, &data);
    second_click(300., 208.);
    second_click(740., 100.);
    assert_eq!(hp(), 32);
    assert_eq!(state.borrow().inventory().quantity(43), 0);
    assert!(!armed(), "exhausting stock ends targeting");
    {
        let mut state = state.borrow_mut();
        state.add_item(44, 2, &data);
        state.stats_mut().set_resource(1, Resource::Hp, 1, &data);
    }
    second_click(300., 208.);
    second_click(740., 210.);
    assert_eq!(hp(), 52, "party items affect every eligible member");
    assert_eq!(
        state.borrow().stats().character(1).unwrap().current().hp,
        21
    );
    assert_eq!(
        state.borrow().inventory().quantity(44),
        1,
        "one party application consumes one copy"
    );
}

#[test]
fn menu_renders_live_party_and_release_latched_inventory_equipment_navigation() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import radiance;
        import shared.openpal3;
        import yaobow.openpal3.state_menu;
        pub fn make() -> &state_menu.MenuState { state_menu.make_menu() }
        pub fn frame(menu: &state_menu.MenuState, ui: &radiance.IUiHost,
            atlas: &openpal3.IPal3UiAtlas, game: &openpal3.IPal3MenuState,
            x: float, y: float, press: bool, release: bool, down: bool) -> bool {
            menu.paint(ui, atlas, game, x, y, press, release, down)
        }
    "#,
    )
    .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(1600, 600);
    let data = Rc::new(menu_data());
    let state = Rc::new(RefCell::new(PersistentState::new("test".into())));
    {
        let mut state = state.borrow_mut();
        state.party_mut().set_in_team(1, true);
        state.stats_mut().set_resource(0, Resource::Hp, 12, &data);
        state.add_money(321);
        state.add_item(42, -2, &data);
        state.add_item(43, 0, &data);
    }
    let service = Pal3MenuState::create(state.clone(), Some(data.clone()));
    let atlas: ComRc<IPal3UiAtlas> = ComRc::from_object(TestAtlas);
    let ui_id = host.intern(ui);
    let atlas_id = host.intern(atlas);
    let game_id = host.intern(service);
    let ui = host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap();
    let atlas = host
        .foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
        .unwrap();
    let game = host
        .foreign_box("shared.openpal3.comdef.IPal3MenuState", game_id)
        .unwrap();
    let menu = host.call_returning_data("make", vec![]).unwrap();
    let frame = |x: f64, y: f64, press: bool, release: bool, down: bool| {
        recorder.calls.borrow_mut().clear();
        host.call_returning_data(
            "frame",
            vec![
                menu.clone(),
                ui.clone(),
                atlas.clone(),
                game.clone(),
                Data::Float(x),
                Data::Float(y),
                Data::Int(press as i64),
                Data::Int(release as i64),
                Data::Int(down as i64),
            ],
        )
        .unwrap()
    };
    let has_text = |text: &str| {
        recorder
            .calls
            .borrow()
            .iter()
            .any(|call| matches!(call, UiCall::TextAt { s, .. } if s == text))
    };

    assert_eq!(frame(0.0, 0.0, false, false, false), Data::Int(1));
    assert!(has_text("12/88"));
    assert!(has_text("321"));
    assert!(has_text("升级所需经验值\n100"));
    assert!(!has_text("206/206"), "inactive Longkui must not appear");
    frame(580.0, 570.0, true, false, true);
    assert!(!has_text("Medicine"), "press alone must not navigate");
    frame(800.0, 400.0, false, true, false);
    assert!(!has_text("Medicine"), "dragging off cancels the click");
    frame(580.0, 570.0, true, false, true);
    frame(580.0, 570.0, false, true, false);
    frame(0.0, 0.0, false, false, false);
    assert!(has_text("Medicine"));
    assert!(has_text("Description"));
    assert!(has_text("价格 17"));
    assert!(has_text("-2"));
    assert!(has_text("Second"));
    state.borrow_mut().remove_item(42, &data);
    frame(0.0, 0.0, false, false, false);
    assert!(
        !has_text("Medicine"),
        "removed selection must not retain stale metadata"
    );
    frame(668.0, 570.0, true, false, true);
    frame(668.0, 570.0, false, true, false);
    frame(1150.0, 210.0, true, false, true);
    frame(1150.0, 210.0, false, true, false);
    frame(0.0, 0.0, false, false, false);
    assert!(
        has_text("Weapon"),
        "equipment selection must read Xuejian's real weapon"
    );
    assert_eq!(
        state.borrow().stats().character(0).unwrap().current().hp,
        12
    );
    for role in 2..5 {
        state.borrow_mut().party_mut().set_in_team(role, true);
    }
    frame(493.0, 570.0, true, false, true);
    frame(493.0, 570.0, false, true, false);
    frame(0.0, 0.0, false, false, false);
    assert!(!has_text("217/217"));
    frame(1050.0, 530.0, true, false, true);
    frame(1050.0, 530.0, false, true, false);
    frame(0.0, 0.0, false, false, false);
    assert!(
        has_text("217/217"),
        "a fifth party member must remain accessible"
    );
    frame(1100.0, 570.0, true, false, true);
    frame(1100.0, 570.0, false, true, false);
    frame(0.0, 0.0, false, false, false);
    assert!(
        has_text("1 - 5 / 36"),
        "System must expose the shared save browser"
    );
    assert!(
        !has_text("217/217"),
        "System must not draw the party resource panel"
    );
    assert!(recorder.calls.borrow().iter().any(|call| matches!(call,
        UiCall::ImageRect { texture_com_id, .. }
            if *texture_com_id == sprite_id("ui/GameMainUI/System/MusicBar.tga"))));
    let unchanged = serde_json::to_value(&*state.borrow()).unwrap();
    for (tab, art, x, y, selected_art) in [
        (3, "Magic/ListFrame.tga", 692., 144., "Magic/Shui2.tga"),
        (4, "Lineup/RoleFrame.tga", 546., 40., "Lineup/Head11.tga"),
        (
            5,
            "Sword/SwordFrame.tga",
            1110.,
            390.,
            "Sword/Sword_Feng.tga",
        ),
        (
            6,
            "JusticeBook/UnUp.tga",
            1142.,
            348.,
            "JusticeBook/Magic1.tga",
        ),
    ] {
        let toolbar_x = 493. + tab as f64 * 88.;
        frame(toolbar_x, 570., true, false, true);
        frame(toolbar_x, 570., false, true, false);
        frame(0., 0., false, false, false);
        assert!(
            recorder.calls.borrow().iter().any(|call| matches!(call,
            UiCall::ImageRect { texture_com_id, .. }
            if *texture_com_id == sprite_id(&format!("ui/GameMainUI/{art}")))),
            "{art}"
        );
        frame(x, y, true, false, true);
        frame(x, y, false, true, false);
        frame(0., 0., false, false, false);
        assert!(
            recorder.calls.borrow().iter().any(|call| matches!(call,
            UiCall::ImageRect { texture_com_id, .. }
            if *texture_com_id == sprite_id(&format!("ui/GameMainUI/{selected_art}")))),
            "{selected_art}"
        );
        frame(800., 300., true, false, true);
        frame(800., 300., false, true, false);
        assert_eq!(
            serde_json::to_value(&*state.borrow()).unwrap(),
            unchanged,
            "visual navigation and unsupported gameplay controls must not alter saves"
        );
    }
    frame(1180.0, 570.0, true, false, true);
    assert_eq!(frame(1180.0, 570.0, false, true, false), Data::Int(0));
}

#[test]
fn native_item_panels_filter_scroll_and_select_without_changing_gameplay() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
            import radiance;
            import shared.openpal3;
            import yaobow.openpal3.state_menu;
            pub fn make() -> &state_menu.MenuState { state_menu.make_menu() }
            pub fn frame(menu: &state_menu.MenuState, ui: &radiance.IUiHost,
                atlas: &openpal3.IPal3UiAtlas, game: &openpal3.IPal3MenuState,
                x: float, y: float, press: bool, release: bool, down: bool) {
                menu.paint(ui, atlas, game, x, y, press, release, down);
            }
        "#,
    )
    .unwrap();
    let names: Vec<_> = (0..14).map(|i| format!("Row{i:02}")).collect();
    let mut items: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (1000 + i as u32, name.as_str(), 1))
        .collect();
    items.extend([
        (42, "Ore", 5),
        (43, "MonsterPart", 12),
        (44, "Unknown", 0),
        (82201, "Weapon", 11),
        (222, "Cloth", 7),
        (111, "Hat", 8),
    ]);
    let data = Rc::new(menu_data_with_items(&items));
    let state = Rc::new(RefCell::new(PersistentState::new("test".into())));
    for &(id, _, _) in &items {
        state
            .borrow_mut()
            .add_item(id as i32, if id == 1000 { -2 } else { 1 }, &data);
    }
    let before = serde_json::to_string(&*state.borrow()).unwrap();
    let service = Pal3MenuState::create(state.clone(), Some(data.clone()));
    let (recorder, ui) = RecordingUiHost::create();
    let ui_id = host.intern(ui);
    let atlas_id = host.intern(ComRc::<IPal3UiAtlas>::from_object(TestAtlas));
    let game_id = host.intern(service);
    let ui = host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap();
    let atlas = host
        .foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
        .unwrap();
    let game = host
        .foreign_box("shared.openpal3.comdef.IPal3MenuState", game_id)
        .unwrap();
    for (width, height, scale, ox, oy) in [
        (800, 600, 1.0, 0.0, 0.0),
        (1600, 600, 1.0, 400.0, 0.0),
        (400, 300, 0.5, 0.0, 0.0),
        (800, 1200, 1.0, 0.0, 300.0),
    ] {
        recorder.set_display_size(width, height);
        let menu = host.call_returning_data("make", vec![]).unwrap();
        let frame = |nx: f64, ny: f64, press: bool, release: bool, down: bool| {
            recorder.calls.borrow_mut().clear();
            host.call_void(
                "frame",
                vec![
                    menu.clone(),
                    ui.clone(),
                    atlas.clone(),
                    game.clone(),
                    Data::Float(ox + nx * scale),
                    Data::Float(oy + ny * scale),
                    Data::Int(press as i64),
                    Data::Int(release as i64),
                    Data::Int(down as i64),
                ],
            )
            .unwrap();
        };
        let click = |nx, ny| {
            frame(nx, ny, true, false, true);
            frame(nx, ny, false, true, false);
            frame(0.0, 0.0, false, false, false);
        };
        let has_row = |name: &str| {
            recorder.calls.borrow().iter().any(|call|
                matches!(call, UiCall::TextAt { x, s, .. } if *x == (ox + 269.0 * scale) as f32 && s == name))
        };
        let info_is = |name: &str| {
            recorder.calls.borrow().iter().any(|call|
                matches!(call, UiCall::TextAt { x, y, s, .. }
                    if *x == (ox + 22.0 * scale) as f32 && *y == (oy + 142.0 * scale) as f32 && s == name))
        };
        click(180.0, 570.0);
        assert!(has_row("Row00") && has_row("Row11") && !has_row("Row12"));
        assert!(info_is("Row00"));
        for (row, name) in names.iter().take(12).enumerate() {
            assert!(recorder.calls.borrow().iter().any(|call| matches!(call,
                UiCall::TextAt { x, y, size: Some(size), s, .. }
                    if s == name && *x == (ox + 269.0 * scale) as f32
                    && *y == (oy + (203.0 + row as f64 * 23.0) * scale) as f32
                    && *size == (16.0 * scale) as f32)));
        }
        for (name, x, y, w, h) in [
            ("ui/GameMainUI/Communal/Info.tga", 10.0, 125.0, 232.0, 414.0),
            (
                "ui/GameMainUI/Equip/ListFrame.tga",
                251.0,
                125.0,
                336.0,
                393.0,
            ),
            ("ui/GameMainUI/Prop/Resume2.tga", 266.0, 123.0, 42.0, 50.0),
            (
                "ui/GameMainUI/Communal/EquipTopFrame/TopFrameL.tga",
                12.0,
                0.0,
                29.0,
                119.0,
            ),
        ] {
            assert!(recorder.calls.borrow().iter().any(|call| matches!(call,
                    UiCall::ImageRect { texture_com_id, x0, y0, x1, y1, .. }
                        if *texture_com_id == sprite_id(name)
                        && (*x0, *y0, *x1, *y1) == ((ox + x * scale) as f32, (oy + y * scale) as f32,
                            (ox + (x + w) * scale) as f32, (oy + (y + h) * scale) as f32))), "{name}");
        }
        frame(300.0, 250.0, true, false, true);
        frame(0.0, 0.0, false, true, false);
        assert!(info_is("Row00"), "dragging off must cancel selection");
        click(300.0, 250.0);
        assert!(info_is("Row02"));
        click(548.0, 460.0);
        assert!(!has_row("Row00") && has_row("Row12"));
        assert!(
            info_is("Row02"),
            "scrolling preserves the selected item's details"
        );
        frame(548.0, 440.0, true, false, true);
        frame(548.0, 440.0, false, true, false);
        assert!(has_row("Row13") && !has_row("Row01"));
        click(437.0, 147.0);
        assert!(has_row("Ore") && has_row("MonsterPart") && !has_row("Unknown"));
        click(390.0, 147.0);
        assert!(
            !info_is("Ore") && !has_row("MonsterPart"),
            "empty filters must clear stale details"
        );
        click(268.0, 570.0);
        assert!(has_row("Weapon") && !has_row("Cloth"));
        click(353.0, 147.0);
        assert!(has_row("Cloth") && !has_row("Weapon"));
        click(419.0, 147.0);
        assert!(has_row("Hat") && !has_row("Cloth"));
        click(300.0, 207.0);
        assert!(
            info_is("Hat"),
            "bag equipment can be inspected without equipping"
        );
    }
    assert_eq!(serde_json::to_string(&*state.borrow()).unwrap(), before);
}

#[test]
fn magic_catalog_selects_and_filters_without_casting_or_consuming_books() {
    let mut database = vec![0u8; 0xa0 + 4 * 760 + 800];
    let header = b"__PAL(Softstar_Ltd: Cool_J)_GameDataBase_OutFile__";
    database[..header.len()].copy_from_slice(header);
    database[0x88..0x8c].copy_from_slice(&0xa0u32.to_le_bytes());
    database[0x8c..0x90].copy_from_slice(&4u32.to_le_bytes());
    database[0x90..0x94].copy_from_slice(&(0xa0u32 + 4 * 760).to_le_bytes());
    database[0x94..0x98].copy_from_slice(&1u32.to_le_bytes());
    for (i, (id, name, kind, element, roles, description)) in [
        (
            301u32,
            "Wind",
            2,
            Some(2),
            [1, 0, 1, 1, 1],
            "$Wind description",
        ),
        (
            302,
            "Heal",
            3,
            Some(2),
            [1; 5],
            "$Healing detail\nsecond\nthird\nfourth\nfifth\nsixth\nlast",
        ),
        (
            316,
            "Water",
            2,
            Some(0),
            [1, 1, 1, 1, 0],
            "$Water description",
        ),
        (
            4101,
            "Steal",
            0,
            None,
            [1, 0, 0, 0, 0],
            "$Skill description",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let record = &mut database[0xa0 + i * 760..0xa0 + (i + 1) * 760];
        record[..4].copy_from_slice(&id.to_le_bytes());
        record[4] = kind;
        if let Some(element) = element {
            record[8 + element * 4..12 + element * 4].copy_from_slice(&1u32.to_le_bytes());
        }
        record[0x1c..0x1c + name.len()].copy_from_slice(name.as_bytes());
        record[0x3c..0x3c + description.len()].copy_from_slice(description.as_bytes());
        record[0x23c..0x241].copy_from_slice(&roles);
    }
    let book = &mut database[0xa0 + 4 * 760..];
    book[..4].copy_from_slice(&6409u32.to_le_bytes());
    book[4..8].copy_from_slice(b"Book");
    book[0x264] = 3;
    book[0x274..0x277].copy_from_slice(&[7, 1, 2]);
    let combat = format!(
        "[Level]\n{}",
        (1..=99)
            .map(|level| format!("{level}={}\n", (level - 1) * 100))
            .collect::<String>()
    );
    let data = Rc::new(StatsData::load(&database, &combat).unwrap());
    let state = Rc::new(RefCell::new(PersistentState::new("test".into())));
    state.borrow_mut().party_mut().set_in_team(1, true);
    state.borrow_mut().grant_world_item(6409, &data);
    assert!(state.borrow_mut().magic_mut().learn(0, 4101, &data));
    let before = serde_json::to_string(&*state.borrow()).unwrap();
    let service = Pal3MenuState::create(state.clone(), Some(data));
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import radiance;
        import shared.openpal3;
        import yaobow.openpal3.state_menu;
        pub fn make() -> &state_menu.MenuState { state_menu.make_menu() }
        pub fn frame(menu: &state_menu.MenuState, ui: &radiance.IUiHost,
            atlas: &openpal3.IPal3UiAtlas, game: &openpal3.IPal3MenuState,
            x: float, y: float, press: bool, release: bool, down: bool) {
            menu.paint(ui, atlas, game, x, y, press, release, down);
        }
    "#,
    )
    .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    let ui_id = host.intern(ui);
    let atlas_id = host.intern(ComRc::<IPal3UiAtlas>::from_object(TestAtlas));
    let game_id = host.intern(service.clone());
    let ui = host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap();
    let atlas = host
        .foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
        .unwrap();
    let game = host
        .foreign_box("shared.openpal3.comdef.IPal3MenuState", game_id)
        .unwrap();
    for (width, height, scale, ox) in [(800, 600, 1.0, 0.0), (1600, 600, 1.0, 400.0)] {
        recorder.set_display_size(width, height);
        let menu = host.call_returning_data("make", vec![]).unwrap();
        let frame = |x: f64, y: f64, press: bool, release: bool, down: bool| {
            recorder.calls.borrow_mut().clear();
            host.call_void(
                "frame",
                vec![
                    menu.clone(),
                    ui.clone(),
                    atlas.clone(),
                    game.clone(),
                    Data::Float(ox + x * scale),
                    Data::Float(y * scale),
                    Data::Int(press as i64),
                    Data::Int(release as i64),
                    Data::Int(down as i64),
                ],
            )
            .unwrap();
        };
        let click = |x, y| {
            frame(x, y, true, false, true);
            frame(x, y, false, true, false);
            frame(0.0, 0.0, false, false, false);
        };
        let has = |text: &str| {
            recorder
                .calls
                .borrow()
                .iter()
                .any(|call| matches!(call, UiCall::TextAt { s, .. } if s == text))
        };
        click(350.0, 570.0);
        assert!(has("Steal") && has("已习得") && has("Skill description"));
        click(420.0, 147.0);
        assert!(has("Wind") && has("Heal") && has("等级1") && has("未习得"));
        assert!(has("Wind description"));
        frame(300.0, 250.0, true, false, true);
        frame(0.0, 0.0, false, true, false);
        assert!(
            has("Wind description"),
            "dragging off must cancel selection"
        );
        click(300.0, 250.0);
        assert!(has("Healing detail") && has("施放尚未接入（只读）"));
        assert!(!has("last"));
        recorder.set_mouse_wheel(-1.0);
        frame(100.0, 450.0, false, false, false);
        recorder.set_mouse_wheel(0.0);
        assert!(has("last") && !has("Healing detail"));
        click(640.0, 220.0);
        assert!(has("Heal") && !has("Wind") && has("Healing detail"));
        click(280.0, 147.0);
        assert!(has("Water") && has("Water description") && !has("Healing detail"));
        click(545.0, 147.0);
        assert!(has("合击数据尚未接入") && !has("Water description"));
    }
    assert!(!service.can_use_item(0, 6409));
    assert!(!service.use_item(0, 6409));
    assert_eq!(state.borrow().inventory().quantity(6409), 1);
    assert_eq!(serde_json::to_string(&*state.borrow()).unwrap(), before);
}

#[test]
fn item_descriptions_wrap_for_the_narrow_native_panel_without_splitting_unicode() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(r#"
            import yaobow.openpal3.state_menu;
            pub fn count(text: string) -> int { state_menu.description_lines(text).len() }
            pub fn line(text: string, index: int) -> string { state_menu.description_lines(text)[index] }
        "#).unwrap();
    for (text, expected) in [
        ("".to_string(), vec![]),
        (
            "first\n\nlast".to_string(),
            vec!["first".to_string(), "".to_string(), "last".to_string()],
        ),
        (
            format!("{}abc", "水".repeat(13)),
            vec!["水".repeat(13), "abc".to_string()],
        ),
        (
            format!("{}水", "a".repeat(25)),
            vec!["a".repeat(25), "水".to_string()],
        ),
    ] {
        assert_eq!(
            host.call_returning_data("count", vec![Data::string(&text)])
                .unwrap(),
            Data::Int(expected.len() as i64)
        );
        for (index, line) in expected.iter().enumerate() {
            assert_eq!(
                host.call_returning_data(
                    "line",
                    vec![Data::string(&text), Data::Int(index as i64)]
                )
                .unwrap(),
                Data::string(line)
            );
        }
    }
}

#[test]
fn menu_resource_fraction_handles_empty_injured_and_overfull_pools() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source_from_path("/yaobow/openpal3/state_menu.p7")
        .unwrap();
    for (current, maximum, expected) in [
        (0, 0, 0.0),
        (12, -1, 0.0),
        (-1, 88, 0.0),
        (22, 88, 0.25),
        (100, 88, 1.0),
    ] {
        let result = host
            .call_returning_data(
                "resource_fraction",
                vec![Data::Int(current), Data::Int(maximum)],
            )
            .unwrap();
        assert_eq!(result, Data::Float(expected));
    }
}
