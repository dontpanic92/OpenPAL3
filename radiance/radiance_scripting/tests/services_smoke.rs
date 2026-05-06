use radiance_scripting::services::{CommandBus, GameRegistry};

#[test]
fn game_registry_exposes_static_game_data() {
    let games = GameRegistry::create();
    assert!(games.count() >= 10);
    assert_eq!(games.game_at(0), 0);
    assert_eq!(games.config_key(0), "OpenPAL3");
    assert!(!games.full_name(0).is_empty());
}

#[test]
fn command_bus_dispatches_and_drains() {
    let bus = CommandBus::create(Some(Box::new(|kind, arg| kind + arg)));
    assert_eq!(bus.dispatch(4, 5), 9);
}
