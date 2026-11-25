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
    comdef::services::{ISprite, ISpriteImpl},
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
        256
    }
    fn height(&self) -> i32 {
        32
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
            record[0x275] = 1;
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
