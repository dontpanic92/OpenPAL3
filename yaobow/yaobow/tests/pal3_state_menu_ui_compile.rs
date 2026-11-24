//! Compile and interaction regressions for the PAL3 status HUD and
//! character-status (状态) menu it delegates to. Loading `status_ui.p7`
//! transitively imports `state_menu.p7`, so this one load exercises both
//! against the production script VFS (so `import radiance_scripting.ui;`
//! and `import yaobow.openpal3.state_menu;` resolve).

use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use p7::interpreter::context::Data;
use radiance_scripting::{
    ScriptHost,
    services::ui_host_recording::{RecordingUiHost, UiCall},
};
use shared::openpal3::{
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

mod openpal3 {
    pub use shared::openpal3::comdef;
}

struct EmptyAtlas;
shared::ComObject_Pal3UiAtlas!(crate::EmptyAtlas);

impl IPal3UiAtlasImpl for EmptyAtlas {
    fn has_sprite(&self, _: &str) -> bool {
        false
    }
    fn sprite(&self, _: &str) -> Option<ComRc<radiance_scripting::comdef::services::ISprite>> {
        None
    }
    fn native_width(&self) -> i32 {
        800
    }
    fn native_height(&self) -> i32 {
        600
    }
}

fn menu_data() -> StatsData {
    let mut database = vec![0u8; 0xa0 + 3 * 800];
    let magic = b"__PAL(Softstar_Ltd: Cool_J)_GameDataBase_OutFile__";
    database[..magic.len()].copy_from_slice(magic);
    database[0x90..0x94].copy_from_slice(&0xa0u32.to_le_bytes());
    database[0x94..0x98].copy_from_slice(&3u32.to_le_bytes());
    for (index, (id, name)) in [(42u32, "Medicine"), (43, "Second"), (82201, "Weapon")]
        .iter()
        .enumerate()
    {
        let record = &mut database[0xa0 + index * 800..0xa0 + (index + 1) * 800];
        record[..4].copy_from_slice(&id.to_le_bytes());
        record[4..4 + name.len()].copy_from_slice(name.as_bytes());
        record[0x60..0x6c].copy_from_slice(b"$Description");
        record[0x260..0x264].copy_from_slice(&17i32.to_le_bytes());
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
    let atlas: ComRc<IPal3UiAtlas> = ComRc::from_object(EmptyAtlas);
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
    frame(1180.0, 570.0, true, false, true);
    assert_eq!(frame(1180.0, 570.0, false, true, false), Data::Int(0));
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
