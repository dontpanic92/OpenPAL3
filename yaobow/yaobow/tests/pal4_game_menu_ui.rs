use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

use crosscom::ComRc;
use p7::interpreter::context::Data;
use radiance_scripting::{
    ScriptHost,
    comdef::services::{IInputService, IInputServiceImpl},
    services::ui_host_recording::{RecordingUiHost, UiCall},
};
use shared::openpal4::{
    gameplay::{
        CharacterStats, EquipmentDefinition, EquipmentType, GameplayCatalog, LevelDefinition,
        PropertyDefinition, RoleDefinition, SkillDefinition,
    },
    menu_state::Pal4MenuState,
    session::Pal4Session,
};
use yaobow_lib::script_source::install_script_assets;

mod comdef {
    pub use radiance_scripting::comdef::services;
}

struct TestInput {
    key: Rc<Cell<i32>>,
}
radiance_scripting::ComObject_InputService!(crate::TestInput);
impl IInputServiceImpl for TestInput {
    fn key_down(&self, key: i32) -> bool {
        self.key.get() == key
    }
    fn key_pressed(&self, key: i32) -> bool {
        self.key.get() == key
    }
    fn mouse_x(&self) -> f32 {
        0.0
    }
    fn mouse_y(&self) -> f32 {
        0.0
    }
    fn axis(&self, _: i32) -> f32 {
        0.0
    }
    fn mouse_button_down(&self, _: i32) -> bool {
        false
    }
    fn mouse_button_pressed(&self, _: i32) -> bool {
        false
    }
    fn mouse_delta_x(&self) -> f32 {
        0.0
    }
    fn mouse_delta_y(&self) -> f32 {
        0.0
    }
    fn mouse_wheel(&self) -> f32 {
        0.0
    }
}

fn catalog() -> Rc<GameplayCatalog> {
    let mut catalog = GameplayCatalog::default();
    let stats = CharacterStats {
        integers: BTreeMap::from([
            ("CA_MAX_HP".into(), 210),
            ("CA_MAX_MP".into(), 30),
            ("CA_PHYSICAL".into(), 24),
        ]),
        ..Default::default()
    };
    for role in 0..4 {
        catalog.roles.insert(
            role,
            RoleDefinition {
                id: role as i32 + 1001,
                name: format!("Role {role}"),
                model: String::new(),
                stats: stats.clone(),
            },
        );
        catalog.levels.insert(
            (role, 1),
            LevelDefinition {
                experience: 54,
                stats: stats.clone(),
            },
        );
    }
    for (id, skill, category) in [
        (3000, 7200, 0),
        (3001, 7201, 0),
        (3013, 7213, 0),
        (3100, 0, 3),
        (3182, 7294, 2),
    ] {
        catalog.properties.insert(
            id,
            PropertyDefinition {
                id,
                name: format!("Item {id}"),
                description: "Recovery description".into(),
                detail: String::new(),
                category,
                target: if id == 3182 { 0 } else { 1 },
                model: String::new(),
                texture: String::new(),
                usable_by: [true; 4],
                usable_in_menu: true,
                droppable: true,
                price: 30,
                skill_id: skill,
            },
        );
    }
    catalog.skills.insert(
        7213,
        SkillDefinition {
            kind: 2,
            trigger: 2,
            duration: 0,
            function: 9118,
            parameters: [1., 0., 0., 2., 0.45, 4., 6., 0.15, 4.],
            attached: 0,
            mutex: 0,
            status: 0,
        },
    );
    catalog.skills.insert(
        7294,
        SkillDefinition {
            kind: 2,
            trigger: 2,
            duration: 0,
            function: 9120,
            parameters: [0.; 9],
            attached: 0,
            mutex: 0,
            status: 0,
        },
    );
    for (id, function) in [(7200, 9118), (7201, 9999)] {
        catalog.skills.insert(
            id,
            SkillDefinition {
                kind: 3,
                trigger: 2,
                duration: 0,
                function,
                parameters: [1.0, 0.0, 0.0, 2.0, 300.0, 0.0, 0.0, 0.0, 0.0],
                attached: 0,
                mutex: 0,
                status: 0,
            },
        );
    }
    for id in [3501, 3502] {
        catalog.equipment.insert(
            id,
            EquipmentDefinition {
                id,
                kind: EquipmentType::Sword,
                name: format!("Sword {id}"),
                description: "Sword".into(),
                model: String::new(),
                texture: String::new(),
                level_limits: [1, 0, 0, 1],
                price: 300,
                skill_id: 0,
                stats: CharacterStats {
                    integers: [("CA_PHYSICAL".into(), 10)].into(),
                    ..Default::default()
                },
            },
        );
    }
    Rc::new(catalog)
}

struct Harness {
    host: Rc<ScriptHost>,
    menu: Data,
    key: Rc<Cell<i32>>,
    state: Rc<RefCell<Pal4Session>>,
    bridge: ComRc<shared::openpal4::comdef::IPal4MenuState>,
}

impl Harness {
    fn new() -> Self {
        Self::in_namespace("OpenPAL4")
    }

    fn in_namespace(namespace: &str) -> Self {
        let host = ScriptHost::new();
        host.set_script_assets(install_script_assets());
        host.load_source(r#"
            import radiance;
            import scripting_services;
            import shared.openpal4;
            import yaobow.openpal4.game_menu;
            import yaobow.openpal4.native_layout;
            import radiance_scripting.ui as gui;
            fn anchor(layout: &native_layout.NativeLayout, name: string, x: float, y: float, w: float, h: float) {
                let mut names: array<string> = [];
                let mut parents: array<int> = [];
                let mut rects: array<gui.Rect> = [];
                for i in Range(0, layout.names.len()) {
                    names.push(layout.names[i]); parents.push(layout.parents[i]); rects.push(layout.rects[i]);
                }
                names.push(name); parents.push(-1); rects.push(gui.Rect(x,y,w,h));
                layout.names = names; layout.parents = parents; layout.rects = rects;
            }
            pub fn make(game: &openpal4.IPal4MenuState, input: &scripting_services.IInputService) -> &game_menu.GameMenu {
                let menu = game_menu.make(game, input, null, null, null, null, null, null, null);
                anchor(menu.portrait, "BtnSystem", 715.0,15.0,68.0,68.0);
                anchor(menu.toolbar, "Role",16.0,524.0,97.0,76.0);
                anchor(menu.toolbar, "Property",118.0,524.0,97.0,76.0);
                anchor(menu.toolbar, "Equipment",219.0,524.0,97.0,76.0);
                anchor(menu.toolbar, "System",620.0,524.0,97.0,76.0);
                anchor(menu.frame, "btnClose",754.0,554.0,40.0,40.0);
                anchor(menu.info, "staticTimeInfo",12.0,81.0,89.0,14.0);
                anchor(menu.info, "staticGameTimeInfo",12.0,124.0,89.0,14.0);
                anchor(menu.status, "qi1",12.0,258.0,140.0,14.0);
                anchor(menu.properties, "PropertyList",377.0,35.0,306.0,189.0);
                anchor(menu.properties, "PropertyClassImage",0.0,0.0,51.0,236.0);
                anchor(menu.properties, "PropertyWindow",338.0,0.0,379.0,240.0);
                anchor(menu.properties, "PropCure",724.0,221.0,65.0,65.0);
                anchor(menu.properties, "PropertyDescription",387.0,250.0,292.0,99.0);
                anchor(menu.properties, "PropertyProp",387.0,381.0,292.0,73.0);
                anchor(menu.equipment, "List",379.0,35.0,304.0,135.0);
                anchor(menu.equipment, "PropertyClassImage",0.0,0.0,51.0,236.0);
                anchor(menu.equipment, "PanelList",338.0,0.0,379.0,199.0);
                anchor(menu.equipment, "touxiang",12.0,333.0,169.0,185.0);
                anchor(menu.system.layout, "List",43.0,38.0,636.0,393.0);
                anchor(menu.system.layout, "BtnSaveData",743.0,187.0,52.0,52.0);
                anchor(menu.system.layout, "BtnLoadData",729.0,278.0,52.0,52.0);
                anchor(menu.system.layout, "BtnSystemSetting",744.0,360.0,52.0,52.0);
                anchor(menu.system.layout, "BtnExit",732.0,437.0,60.0,60.0);
                anchor(menu.system.layout, "ScroolAudio1",254.0,38.0,323.0,38.0);
                anchor(menu.system.layout, "ScroolAudio2",255.0,76.0,323.0,38.0);
                for i in Range(0,10) {
                    anchor(menu.system.layout, "BtnData"+if i==0 { "0" } else {
                        let digits: array<string> = ["0","1","2","3","4","5","6","7","8","9"]; digits[i]
                    },43.0+(i as float)*63.0,456.0,63.0,51.0);
                }
                anchor(menu.system.dialog, "BtYes",43.0,77.0,60.0,26.0);
                anchor(menu.system.dialog, "BtNo",172.0,77.0,60.0,26.0);
                anchor(menu.system.dialog, "BtOk",107.0,77.0,60.0,26.0);
                menu
            }
            pub fn tick(menu: &game_menu.GameMenu) -> int { menu.tick(); menu.selected }
            pub fn frame(menu: &game_menu.GameMenu, ui: &radiance.IUiHost, x: float, y: float, down: bool) -> int {
                menu.paint_at(ui, x, y, down); menu.selected
            }
        "#).expect("PAL4 menu must compile");
        let catalog = catalog();
        let mut session = Pal4Session::from_state(
            shared::openpal4::states::persistent_state::Pal4PersistentState::new(namespace.into()),
        );
        session.install_catalog(catalog.clone()).unwrap();
        session.set_menu_host_available(true);
        session.state_mut().set_player_locked(false);
        session.state_mut().player_mut(0).hp = 10;
        for id in [3100, 3001, 3000] {
            session.state_mut().add_property(id, 2, &catalog).unwrap();
        }
        session
            .state_mut()
            .add_player_equip(0, 3501, &catalog)
            .unwrap();
        session.state_mut().add_equipment(3502, 1);
        let state = Rc::new(RefCell::new(session));
        let bridge = Pal4MenuState::create(state.clone());
        bridge.inner::<Pal4MenuState>().set_available(true);
        let key = Rc::new(Cell::new(-1));
        let input: ComRc<IInputService> = ComRc::from_object(TestInput { key: key.clone() });
        let game_id = host.intern(bridge.clone());
        let input_id = host.intern(input);
        let game = host
            .foreign_box("shared.openpal4.comdef.IPal4MenuState", game_id)
            .unwrap();
        let input = host
            .foreign_box("radiance_scripting.comdef.services.IInputService", input_id)
            .unwrap();
        let menu = host.call_returning_data("make", vec![game, input]).unwrap();
        Self {
            host,
            menu,
            key,
            state,
            bridge,
        }
    }
    fn key(&self, key: i32) {
        self.key.set(key);
        self.host
            .call_returning_data("tick", vec![self.menu.clone()])
            .unwrap();
        self.key.set(-1);
    }
    fn frame(&self, ui: Data, x: f64, y: f64, down: bool) {
        self.host
            .call_returning_data(
                "frame",
                vec![
                    self.menu.clone(),
                    ui,
                    Data::Float(x),
                    Data::Float(y),
                    Data::Int(down as i64),
                ],
            )
            .unwrap();
    }

    fn click(&self, ui: &Data, x: f64, y: f64) {
        self.frame(ui.clone(), x, y, true);
        self.frame(ui.clone(), x, y, false);
    }
}

struct SystemSaves(std::path::PathBuf);
impl SystemSaves {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "yaobow-pal4-system-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )))
    }
    fn namespace(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for SystemSaves {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

#[test]
fn system_pages_distinguish_empty_corrupt_and_valid_saves_and_gate_overwrites() {
    let saves = SystemSaves::new();
    let h = Harness::in_namespace(saves.namespace());
    h.state
        .borrow_mut()
        .state_mut()
        .set_scene("M02".into(), "1".into());
    h.state.borrow().state().try_save(1).unwrap();
    h.state.borrow().state().try_save(100).unwrap();
    std::fs::write(saves.0.join("Save").join("2.json"), b"broken save").unwrap();
    assert!(h.bridge.open(6));
    h.key(-1);
    assert_eq!(
        (
            h.bridge.slot_state(1),
            h.bridge.slot_state(2),
            h.bridge.slot_state(3)
        ),
        (1, 2, 0)
    );
    assert!(h.bridge.slot_summary(100).contains("M02"));
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(1600, 600);
    let id = h.host.intern(ui);
    let ui = h.host.foreign_box("radiance.comdef.IUiHost", id).unwrap();
    h.frame(ui.clone(), 0., 0., false);
    h.click(&ui, 500., 85.);
    assert!(
        !h.bridge.system_busy(),
        "occupied slots must not save before confirmation"
    );
    h.click(&ui, 1060., 560.);
    h.click(&ui, 1174., 574.);
    assert_eq!(
        h.bridge.active_tab(),
        6,
        "confirmation must block tabs and close"
    );
    h.key(38);
    assert_eq!(
        h.bridge.active_tab(),
        6,
        "Escape cancels the confirmation, not the menu"
    );
    h.key(41);
    recorder.calls.borrow_mut().clear();
    h.frame(ui.clone(), 0., 0., false);
    assert!(
        recorder
            .calls
            .borrow()
            .iter()
            .any(|c| matches!(c, UiCall::TextAt { s, .. } if s == "11"))
    );
    for _ in 0..8 {
        h.key(41);
    }
    for _ in 0..9 {
        h.key(42);
    }
    h.key(0);
    assert!(!h.bridge.system_busy());
    h.click(&ui, 400. + 263.5 + 65., 230. + 90.);
    assert!(
        h.bridge.system_busy(),
        "explicit checkmark submits the occupied slot"
    );
}

#[test]
fn native_save_rows_show_snapshot_details_and_scroll_to_the_selected_slot() {
    let saves = SystemSaves::new();
    let h = Harness::in_namespace(saves.namespace());
    {
        let mut session = h.state.borrow_mut();
        let state = session.state_mut();
        state.set_scene("M02".into(), "3".into());
        state.player_mut(0).level = 22;
        state.advance_play_time(3661.);
        state.add_quest_percentage(42);
        state.try_save(2).unwrap();
        state.try_save(10).unwrap();
    }
    assert!(h.bridge.open(6));
    h.key(-1);
    let snapshot = h.bridge.save_slots();
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(800, 600);
    let id = h.host.intern(ui);
    let ui = h.host.foreign_box("radiance.comdef.IUiHost", id).unwrap();
    h.frame(ui.clone(), 0., 0., false);
    assert!(
        recorder
            .calls
            .borrow()
            .iter()
            .any(|call| matches!(call, UiCall::TextAt { s, .. } if s == "22级"))
    );
    assert!(
        recorder
            .calls
            .borrow()
            .iter()
            .any(|call| matches!(call, UiCall::TextAt { s, .. } if s == "01:01:01"))
    );
    h.click(&ui, 660., 419.);
    h.key(-1);
    recorder.calls.borrow_mut().clear();
    h.frame(ui.clone(), 0., 0., false);
    assert!(
        !recorder
            .calls
            .borrow()
            .iter()
            .any(|call| matches!(call, UiCall::TextAt { s, .. } if s == "1")),
        "mouse scrolling must persist on an idle tick"
    );
    h.key(40);
    recorder.set_mouse_wheel(-1.);
    h.frame(ui.clone(), 400., 170., false);
    recorder.set_mouse_wheel(0.);
    h.key(-1);
    recorder.calls.borrow_mut().clear();
    h.frame(ui.clone(), 400., 170., false);
    assert!(
        !recorder
            .calls
            .borrow()
            .iter()
            .any(|call| matches!(call, UiCall::TextAt { s, .. } if s == "1")),
        "wheel scrolling must persist on an idle tick"
    );
    h.key(40);
    h.click(&ui, 400., 170.);
    assert!(
        !h.bridge.system_busy(),
        "selecting a different row must not save or load"
    );
    for _ in 0..8 {
        h.key(42);
    }
    recorder.calls.borrow_mut().clear();
    h.frame(ui.clone(), 0., 0., false);
    assert!(
        recorder
            .calls
            .borrow()
            .iter()
            .any(|call| matches!(call, UiCall::TextAt { s, .. } if s == "10"))
    );
    assert!(
        !recorder
            .calls
            .borrow()
            .iter()
            .any(|call| matches!(call, UiCall::TextAt { s, .. } if s == "1"))
    );
    h.state.borrow_mut().state_mut().player_mut(0).level = 23;
    h.state.borrow().state().try_save(10).unwrap();
    h.bridge.refresh_slots();
    assert_eq!(h.bridge.save_slots().level(10), 23);
    assert_eq!(
        snapshot.level(10),
        22,
        "refresh must not change a live snapshot"
    );
}

#[test]
fn portable_save_opens_native_system_page_and_spends_once_even_when_cancelled() {
    let saves = SystemSaves::new();
    let h = Harness::in_namespace(saves.namespace());
    let catalog = h.state.borrow().catalog();
    h.state
        .borrow_mut()
        .state_mut()
        .add_property(3182, 2, &catalog)
        .unwrap();
    h.key(38);
    h.key(28);
    assert!(h.bridge.can_use(0, 3182));
    // The newest backpack row is the portable-save item.
    h.key(0);
    assert_eq!(h.bridge.active_tab(), 6);
    assert_eq!(h.bridge.item_quantity(3182), 1);
    assert!(
        !h.bridge.use_item(0, 3182),
        "no repeated use after switching tabs"
    );
    h.key(-1);
    h.key(38);
    assert_eq!(h.bridge.active_tab(), -1);
    assert_eq!(
        h.bridge.item_quantity(3182),
        1,
        "cancelling does not refund native use"
    );
    assert!(
        !saves.0.join("Save").exists(),
        "opening save UI does not create a save"
    );
}

#[test]
fn portable_save_respects_story_locks_menu_grants_and_live_availability() {
    let h = Harness::new();
    let catalog = h.state.borrow().catalog();
    h.state
        .borrow_mut()
        .state_mut()
        .add_property(3182, 1, &catalog)
        .unwrap();
    h.key(38);
    h.key(28);
    h.state.borrow_mut().state_mut().grant_menu(6, false);
    assert!(!h.bridge.use_item(0, 3182));
    h.state.borrow_mut().state_mut().grant_menu(6, true);
    h.state.borrow_mut().state_mut().set_player_locked(true);
    assert!(!h.bridge.use_item(0, 3182));
    h.state.borrow_mut().state_mut().set_player_locked(false);
    h.bridge.inner::<Pal4MenuState>().set_available(false);
    assert!(!h.bridge.use_item(0, 3182));
    assert!(h.state.borrow().request_menu(1, true));
    assert!(
        !h.bridge.use_item(0, 3182),
        "forced script menus cannot navigate away"
    );
    h.bridge.inner::<Pal4MenuState>().set_available(true);
    assert!(!h.bridge.can_use(0, 3182));
    assert_eq!(h.bridge.item_quantity(3182), 1);
}

#[test]
fn adventure_food_rounds_scaled_recovery_as_native_float_before_consumption() {
    let h = Harness::new();
    let catalog = h.state.borrow().catalog();
    {
        let mut session = h.state.borrow_mut();
        session.state_mut().add_property(3013, 2, &catalog).unwrap();
        let player = session.state_mut().player_mut(0);
        player.max_hp = 200;
        player.hp = 1;
        player.mp = 0;
    }
    h.key(38);
    h.key(28);
    h.key(0);
    let player = h.state.borrow().state().player(0).unwrap().clone();
    assert_eq!((player.hp, player.mp), (91, 4));
    assert_eq!(h.bridge.item_quantity(3013), 1);
    h.state.borrow_mut().state_mut().player_mut(0).hp = 0;
    assert!(
        !h.bridge.use_item(0, 3013),
        "ordinary food cannot revive or heal a dead role"
    );
    assert_eq!(h.bridge.item_quantity(3013), 1);
    assert_eq!(h.state.borrow().state().player(0).unwrap().mp, 4);
}

#[test]
fn system_load_rejects_bad_slots_and_exit_choices_require_confirmation() {
    let saves = SystemSaves::new();
    let h = Harness::in_namespace(saves.namespace());
    assert!(h.bridge.open(6));
    h.key(-1);
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(800, 600);
    let id = h.host.intern(ui);
    let ui = h.host.foreign_box("radiance.comdef.IUiHost", id).unwrap();
    h.frame(ui.clone(), 0., 0., false);
    h.click(&ui, 750., 300.);
    h.key(0);
    assert!(!h.bridge.system_busy());
    h.key(38);
    h.click(&ui, 750., 460.);
    h.click(&ui, 300., 215.);
    assert!(!h.bridge.system_busy());
    h.click(&ui, 263.5 + 200., 230. + 90.);
    assert!(!h.bridge.system_busy(), "cross cancels return-to-title");
    h.click(&ui, 300., 285.);
    h.key(0);
    assert!(
        !h.bridge.system_busy(),
        "Space must not accept a destructive default"
    );
    h.click(&ui, 263.5 + 65., 230. + 90.);
    assert!(h.bridge.system_busy());
}

#[test]
fn system_audio_sliders_clamp_independently_and_surface_persistence_failure() {
    let saves = SystemSaves::new();
    let h = Harness::in_namespace(saves.namespace());
    assert!(h.bridge.open(6));
    h.key(-1);
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(800, 600);
    let id = h.host.intern(ui);
    let ui = h.host.foreign_box("radiance.comdef.IUiHost", id).unwrap();
    h.frame(ui.clone(), 0., 0., false);
    h.click(&ui, 760., 380.);
    h.frame(ui.clone(), 400., 52., true);
    h.frame(ui.clone(), 200., 52., true);
    h.frame(ui.clone(), 200., 52., false);
    assert_eq!(h.bridge.audio_volume(true), 0.);
    assert_eq!(h.bridge.audio_volume(false), 1.);
    assert!(
        recorder
            .calls
            .borrow()
            .iter()
            .any(|c| matches!(c, UiCall::TextAt { s,.. } if s.contains("保存失败")))
    );
    assert!(!h.bridge.set_audio_volume(false, f32::NAN));
    h.key(38);
    assert_eq!(h.bridge.active_tab(), 6);
    h.frame(ui.clone(), 400., 95., true);
    h.frame(ui.clone(), 60., 560., true);
    assert_eq!(
        h.bridge.active_tab(),
        6,
        "slider capture blocks toolbar changes"
    );
    h.key(38);
    assert_eq!(
        h.bridge.active_tab(),
        6,
        "Escape ends a captured drag before closing"
    );
    assert_eq!(
        h.bridge.audio_volume(false),
        1.,
        "cancelling restores the captured gain"
    );
    h.frame(ui.clone(), 0., 0., false);
    h.key(38);
    assert_eq!(h.bridge.active_tab(), -1);
    assert!(!h.bridge.set_audio_volume(false, 0.));
}

#[test]
fn native_menu_displays_saved_and_current_session_time_separately() {
    let h = Harness::new();
    h.state.borrow_mut().advance_play_time(3661.0);
    let saved = h.state.borrow().state().clone();
    h.state.borrow_mut().advance_play_time(60.0);
    h.state.borrow_mut().replace_state(saved);
    h.key(38);
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(800, 600);
    let id = h.host.intern(ui);
    let ui = h.host.foreign_box("radiance.comdef.IUiHost", id).unwrap();
    h.frame(ui, 0.0, 0.0, false);
    let calls = recorder.calls.borrow();
    assert!(
        calls
            .iter()
            .any(|call| matches!(call, UiCall::TextAt { s, .. } if s == "01:01:01"))
    );
    assert!(
        calls
            .iter()
            .any(|call| matches!(call, UiCall::TextAt { s, .. } if s == "01:02:01"))
    );
}

#[test]
fn menu_new_category_is_real_history_and_rage_is_authoritative() {
    let h = Harness::new();
    assert_eq!(h.bridge.inventory_item(1, -1, 0), 3000);
    let catalog = h.state.borrow().catalog();
    h.state
        .borrow_mut()
        .state_mut()
        .add_property(3100, 1, &catalog)
        .unwrap();
    assert_eq!(h.bridge.inventory_item(1, -1, 0), 3100);
    assert_eq!(h.bridge.inventory_item(1, 0, 0), 3000);
    h.state.borrow_mut().state_mut().player_mut(0).rage = 37;
    h.key(38);
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(800, 600);
    let id = h.host.intern(ui);
    let ui = h.host.foreign_box("radiance.comdef.IUiHost", id).unwrap();
    h.frame(ui, 0., 0., false);
    assert!(
        recorder
            .calls
            .borrow()
            .iter()
            .any(|c| matches!(c,UiCall::TextAt { s,.. } if s=="37/100"))
    );
}

#[test]
fn keyboard_menu_uses_and_equips_authoritative_state_without_wasting_items() {
    let h = Harness::new();
    h.key(38);
    assert_eq!(h.bridge.active_tab(), 0);
    assert!(h.state.borrow().menu_input_blocked());
    h.key(28);
    assert_eq!(h.bridge.active_tab(), 1);
    h.key(0);
    assert_eq!(h.state.borrow().state().player(0).unwrap().hp, 210);
    assert_eq!(h.bridge.item_quantity(3000), 1);
    h.key(0);
    assert_eq!(h.bridge.item_quantity(3000), 1);
    h.key(42);
    h.key(0);
    assert_eq!(h.bridge.item_quantity(3001), 2);
    h.key(29);
    h.key(0);
    assert_eq!(h.bridge.equipped_item(0, 0), 3502);
    assert_eq!(h.bridge.item_quantity(3501), 1);
    assert_eq!(h.bridge.item_quantity(3502), 0);
    h.key(38);
    assert_eq!(h.bridge.active_tab(), -1);
    assert!(!h.bridge.equip(0, 3501));
    assert!(!h.state.borrow().state().player_locked());
}

#[test]
fn mouse_press_latching_respects_letterboxing_and_disabled_tabs() {
    let h = Harness::new();
    let (recorder, ui) = RecordingUiHost::create();
    let id = h.host.intern(ui);
    let ui = h.host.foreign_box("radiance.comdef.IUiHost", id).unwrap();
    recorder.set_display_size(1600, 600);
    h.key(38);
    h.frame(ui.clone(), 0.0, 0.0, false);
    h.state.borrow_mut().state_mut().grant_menu(1, false);
    h.frame(ui.clone(), 560.0, 560.0, true);
    h.frame(ui.clone(), 560.0, 560.0, false);
    assert_eq!(h.bridge.active_tab(), 0);
    h.state.borrow_mut().state_mut().grant_menu(1, true);
    h.frame(ui.clone(), 560.0, 560.0, true);
    h.frame(ui.clone(), 0.0, 0.0, false);
    assert_eq!(h.bridge.active_tab(), 0);
    h.frame(ui.clone(), 560.0, 560.0, true);
    h.frame(ui.clone(), 560.0, 560.0, false);
    assert_eq!(h.bridge.active_tab(), 1);
    assert!(!recorder.calls.borrow().is_empty());
    assert!(
        !recorder
            .calls
            .borrow()
            .iter()
            .any(|call| matches!(call, UiCall::FillRect { a, .. } if *a > 0.8))
    );
}

#[test]
fn native_portrait_opens_menu_and_native_close_consumes_input_at_multiple_sizes() {
    for (width, height, ox, scale) in [
        (800, 600, 0.0, 1.0),
        (1600, 600, 400.0, 1.0),
        (1600, 1200, 0.0, 2.0),
    ] {
        let h = Harness::new();
        let (recorder, ui) = RecordingUiHost::create();
        recorder.set_display_size(width, height);
        let id = h.host.intern(ui);
        let ui = h.host.foreign_box("radiance.comdef.IUiHost", id).unwrap();
        h.frame(ui.clone(), 0.0, 0.0, false);
        h.frame(ui.clone(), ox + 748.0 * scale, 48.0 * scale, true);
        h.frame(ui.clone(), ox + 748.0 * scale, 48.0 * scale, false);
        assert_eq!(h.bridge.active_tab(), 0);
        h.frame(ui.clone(), 0.0, 0.0, false);
        // The removed custom top tab must no longer accept clicks.
        h.frame(ui.clone(), ox + 175.0 * scale, 42.0 * scale, true);
        h.frame(ui.clone(), ox + 175.0 * scale, 42.0 * scale, false);
        assert_eq!(h.bridge.active_tab(), 0);
        h.frame(ui.clone(), ox + 774.0 * scale, 574.0 * scale, true);
        h.frame(ui.clone(), ox + 774.0 * scale, 574.0 * scale, false);
        assert_eq!(h.bridge.active_tab(), -1);
        assert!(h.state.borrow().menu_input_blocked());
        h.bridge.inner::<Pal4MenuState>().set_available(false);
        h.frame(ui.clone(), ox + 748.0 * scale, 48.0 * scale, true);
        h.frame(ui, ox + 748.0 * scale, 48.0 * scale, false);
        assert_eq!(h.bridge.active_tab(), -1);
    }
}

#[test]
fn native_inventory_rows_use_items_only_after_a_completed_double_click() {
    let h = Harness::new();
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(800, 600);
    let id = h.host.intern(ui);
    let ui = h.host.foreign_box("radiance.comdef.IUiHost", id).unwrap();
    h.key(38);
    h.key(28);
    h.frame(ui.clone(), 0.0, 0.0, false);
    h.frame(ui.clone(), 410.0, 45.0, true);
    h.frame(ui.clone(), 200.0, 400.0, false);
    assert_eq!(h.bridge.item_quantity(3000), 2);
    h.frame(ui.clone(), 410.0, 45.0, true);
    h.frame(ui.clone(), 410.0, 45.0, false);
    assert_eq!(h.bridge.item_quantity(3000), 2);
    h.frame(ui.clone(), 410.0, 45.0, true);
    h.frame(ui, 410.0, 45.0, false);
    assert_eq!(h.bridge.item_quantity(3000), 1);
    assert_eq!(h.state.borrow().state().player(0).unwrap().hp, 210);
}

#[test]
fn app_factory_and_actor_controller_compile_with_menu_contracts() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source_from_path("/yaobow/app.p7").unwrap();
}

#[test]
fn forced_script_menu_can_close_without_unlocking_cutscene_control() {
    let h = Harness::new();
    h.state.borrow_mut().state_mut().set_player_locked(true);
    h.bridge.inner::<Pal4MenuState>().set_available(false);
    h.key(38);
    assert_eq!(h.bridge.active_tab(), -1);
    assert!(h.state.borrow().request_menu(1, true));
    assert!(h.bridge.can_use(0, 3000));
    h.key(0);
    assert_eq!(h.bridge.item_quantity(3000), 1);
    h.key(38);
    assert_eq!(h.bridge.active_tab(), -1);
    assert!(h.state.borrow().state().player_locked());
}
