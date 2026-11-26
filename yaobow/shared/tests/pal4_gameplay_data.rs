use std::rc::Rc;

use packfs::init_virtual_fs;
use shared::openpal4::{
    gameplay::GameplayCatalog,
    session::Pal4Session,
    states::persistent_state::{PAL4_APP_NAME, Pal4PersistentState},
};

#[test]
#[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
fn original_database_initializes_and_persists_gameplay() {
    let root = std::env::var("PAL4_ASSET_ROOT").expect("set PAL4_ASSET_ROOT");
    let vfs = init_virtual_fs(&root, None);
    let catalog = Rc::new(GameplayCatalog::load(&vfs).unwrap());
    assert_eq!((catalog.roles.len(), catalog.levels.len()), (4, 396));
    assert_eq!(
        (catalog.properties.len(), catalog.equipment.len()),
        (246, 188)
    );
    assert_eq!(catalog.roles[&0].stats.integer("CA_MAX_HP"), 90);

    let mut session = Pal4Session::new();
    session.install_catalog(catalog.clone()).unwrap();
    let state = session.state_mut();
    assert_eq!(
        (state.player(0).unwrap().hp, state.player(0).unwrap().mp),
        (210, 30)
    );
    state.add_property(3000, i32::MAX, &catalog).unwrap();
    assert_eq!(state.equipment_count(3000), 99);
    state.add_property(3000, -1, &catalog).unwrap();
    assert_eq!(state.equipment_count(3000), 98);
    state.add_property(3000, i32::MIN, &catalog).unwrap();
    assert!(!state.has_equipment(3000));
    assert!(state.add_property(3501, 1, &catalog).is_err());
    assert!(state.add_property(9999, 1, &catalog).is_err());
    state.add_player_equip(0, 3586, &catalog).unwrap();
    state.add_player_equip(0, 3501, &catalog).unwrap();
    assert_eq!(
        state
            .player(0)
            .unwrap()
            .effective_stats
            .integer("CA_PHYSICAL"),
        34
    );
    state.set_in_team(1, true);
    state.set_player_level(1, 2, &catalog).unwrap();
    state.add_money(250);
    state.set_player_resource_percent(-1, -1, 50);
    let restored: Pal4PersistentState =
        serde_json::from_str(&serde_json::to_string(state).unwrap()).unwrap();
    assert_eq!(restored.app_name(), PAL4_APP_NAME);
    assert_eq!(restored.money(), 250);
    assert!(restored.player(1).unwrap().in_team);
    assert_eq!(restored.player(1).unwrap().level, 2);
    assert_eq!(
        restored.player(0).unwrap().equipment,
        state.player(0).unwrap().equipment
    );
    assert_eq!(restored.player(0).unwrap().hp, state.player(0).unwrap().hp);
    let mut session = Pal4Session::from_state(restored);
    session.install_catalog(catalog).unwrap();
    assert_eq!(session.state().player(0).unwrap().hp, 105);
    session.new_game().unwrap();
    assert_eq!(session.state().money(), 0);
    assert_eq!(session.state().player(0).unwrap().hp, 210);
    assert_eq!(
        session.state().player(0).unwrap().equipment.ids().count(),
        0
    );
}
