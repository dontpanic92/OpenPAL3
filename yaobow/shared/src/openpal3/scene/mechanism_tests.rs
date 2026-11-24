use super::ScnScene;
use crate::openpal3::{
    asset_manager::AssetManager,
    loaders::{
        nav_loader::{NavFile, NavMap, NavMapPoint},
        scn_loader::{ScnFile, ScnNode},
    },
    states::persistent_state::PersistentState,
};
use radiance::{
    comdef::{IAnimatedMeshComponent, IComponent, IEntityExt, IStaticMeshComponent},
    components::mesh::{AnimatedMeshComponent, Geometry, MorphTarget, StaticMeshComponent},
    math::Vec3,
    rendering::{
        ComponentFactory, MaterialDef, RenderObjectHandle, RenderTarget, RenderingComponent,
        ShaderProgram, Texture, TextureDef, VertexBuffer, VertexComponents, VideoPlayer,
    },
    scene::{CoreEntity, CoreScene},
};
use std::rc::Rc;

struct UnusedFactory;

impl ComponentFactory for UnusedFactory {
    fn create_texture(&self, _: &TextureDef) -> Box<dyn Texture> {
        unreachable!()
    }
    fn create_imgui_texture(
        &self,
        _: &[u8],
        _: u32,
        _: u32,
        _: u32,
        _: Option<imgui::TextureId>,
    ) -> (Box<dyn Texture>, imgui::TextureId) {
        unreachable!()
    }
    fn remove_imgui_texture(&self, _: Option<imgui::TextureId>) {
        unreachable!()
    }
    fn create_render_object(
        &self,
        _: VertexBuffer,
        _: Vec<u32>,
        _: &MaterialDef,
        _: bool,
    ) -> RenderObjectHandle {
        unreachable!()
    }
    fn create_rendering_component(&self, objects: Vec<RenderObjectHandle>) -> RenderingComponent {
        assert!(objects.is_empty());
        RenderingComponent::new()
    }
    fn create_video_player(&self) -> Box<VideoPlayer> {
        unreachable!()
    }
    fn create_render_target(&self, _: u32, _: u32) -> Box<dyn RenderTarget> {
        unreachable!()
    }
}

fn node(id: u16, kind: u16, target: u16) -> ScnNode {
    ScnNode {
        index: id,
        active: 1,
        remaining_triggers: 255,
        switch_state: 0,
        name: String::new(),
        w24: 0,
        w26: 0,
        position: Vec3::new(0., 10., 0.),
        rotation: 0.,
        nav_trigger_coord_min: (0, 0),
        nav_trigger_coord_max: (0, 0),
        node_type: kind,
        nav_layer: 0,
        ladder_nav_coord1: (40, if kind == 23 { 1 } else { 0 }),
        ladder_nav_coord2: (0, 0),
        ladder_switch_layer: 0,
        b49: vec![],
        w66: vec![],
        b6e: vec![],
        sce_proc_id: 0,
        d84: 0,
        b88: vec![],
        w148: target,
        b14a: vec![0; 34],
        aabb_trigger_coord1: Vec3::new(0., 0., 0.),
        aabb_trigger_coord2: Vec3::new(0., 0., 0.),
        dw184: vec![],
        b: vec![],
    }
}

fn scene(nodes: Vec<ScnNode>) -> ScnScene {
    scene_with_nav(
        nodes,
        "m17",
        NavFile {
            version: 1,
            maps: vec![],
            unknown2: vec![],
        },
        crate::GameType::PAL3,
    )
}

fn scene_with_nav(nodes: Vec<ScnNode>, cpk: &str, nav: NavFile, game: crate::GameType) -> ScnScene {
    let scene = CoreScene::create();
    scene.add_entity(CoreEntity::create("ROLE_0".into(), true));
    for node in &nodes {
        let entity = CoreEntity::create(format!("OBJECT_{}", node.index), true);
        entity.transform().borrow_mut().set_position(&node.position);
        scene.add_entity(entity);
    }
    let assets = Rc::new(AssetManager::new_for_game(
        Rc::new(UnusedFactory),
        Rc::new(mini_fs::MiniFs::new(true)),
        game,
    ));
    ScnScene::new(
        scene,
        &assets,
        cpk,
        "7",
        ScnFile {
            cpk_name: cpk.into(),
            scn_name: "7".into(),
            scn_base_name: "7".into(),
            global_scene_index: 0,
            is_night: false,
            skybox_id: 0,
            roles: vec![],
            nodes,
        },
        nav,
    )
}

#[test]
fn mechanism_links_wait_for_completion_and_survive_scene_restore() {
    let switch = node(9, 23, 3);
    let mut platform = node(3, 22, u16::MAX);
    platform.sce_proc_id = 507;
    let scene = scene(vec![switch.clone(), platform.clone()]);
    scene.trigger_mechanism(9);
    assert_eq!(scene.switch_state(9), 1);
    assert!(scene.update_mechanisms(1.).is_empty());
    let height = || {
        scene
            .get_root_object(3)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y
    };
    assert_eq!(height(), 10.);
    assert!(scene.update_mechanisms(0.5).is_empty());
    assert_eq!(height(), 25.);
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let restored = self::scene(vec![switch, platform]);
    restored.restore_mechanisms(&saved);
    assert_eq!(restored.switch_state(9), 1);
    assert_eq!(restored.update_mechanisms(0.5), vec![507]);
    assert_eq!(
        restored
            .get_root_object(3)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y,
        40.
    );
    assert!(restored.update_mechanisms(1.).is_empty());
}

#[test]
fn mechanism_self_links_are_deferred_instead_of_recursing() {
    let scene = scene(vec![node(9, 23, 9)]);
    scene.trigger_mechanism(9);
    assert_eq!(scene.switch_state(9), 1);
    scene.update_mechanisms(1.);
    assert_eq!(scene.switch_state(9), 0);
    scene.update_mechanisms(1.);
    assert_eq!(scene.switch_state(9), 1);
}

#[test]
fn mechanism_interaction_respects_layer_distance_and_runtime_visibility() {
    let mut switch = node(9, 23, u16::MAX);
    switch.ladder_nav_coord1 = (1, 2);
    switch.sce_proc_id = 500;
    let scene = scene(vec![switch]);
    let near = Vec3::new(0., 10., 0.);
    assert_eq!(scene.interact_mechanism(1, &near), None);
    assert_eq!(scene.interact_mechanism(0, &Vec3::new(201., 10., 0.)), None);
    assert_eq!(scene.interact_mechanism(0, &near), Some(Some(500)));
    assert_eq!(scene.switch_state(9), 0);
    scene.get_root_object(9).unwrap().set_visible(false);
    assert_eq!(scene.interact_mechanism(0, &near), None);
    scene.trigger_mechanism(9);
    assert_eq!(scene.switch_state(9), 1);
}

#[test]
fn mechanism_quarter_turn_chain_waits_for_each_rotation_and_does_not_run_object_procs() {
    let mut first = node(2, 7, 3);
    first.rotation = 180.;
    first.sce_proc_id = 500;
    let scene = scene(vec![first, node(3, 7, 4), node(4, 7, u16::MAX)]);
    scene.trigger_mechanism(2);
    assert!(scene.update_mechanisms(1.).is_empty());
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let states = saved.mechanism_states("m17", "7").unwrap();
    assert_eq!(states[&2].native.as_ref().unwrap().rotation, 210.);
    assert_eq!(states[&3].native.as_ref().unwrap().rotation, 0.);
    assert_eq!(states[&3].native.as_ref().unwrap().rotation_target, None);
    assert!(scene.update_mechanisms(2.).is_empty());
    scene.snapshot_mechanisms(&mut saved);
    let states = saved.mechanism_states("m17", "7").unwrap();
    assert_eq!(states[&2].native.as_ref().unwrap().rotation, 270.);
    assert_eq!(
        states[&3].native.as_ref().unwrap().rotation_target,
        Some(90.)
    );
    assert_eq!(states[&4].native.as_ref().unwrap().rotation_target, None);
    assert!(scene.update_mechanisms(3.).is_empty());
    scene.snapshot_mechanisms(&mut saved);
    assert_eq!(
        saved.mechanism_states("m17", "7").unwrap()[&4]
            .native
            .as_ref()
            .unwrap()
            .rotation_target,
        Some(90.)
    );
}

fn gate_nav() -> NavFile {
    NavFile {
        version: 1,
        maps: vec![NavMap {
            layer_triggers: None,
            max_coord: Vec3::new(80., 0., 20.),
            min_coord: Vec3::new(0., 0., 0.),
            width: 3,
            height: 1,
            map: vec![vec![
                NavMapPoint {
                    height: 0.,
                    distance_to_border: 0x0200,
                },
                NavMapPoint {
                    height: 0.,
                    distance_to_border: 0x0301,
                },
                NavMapPoint {
                    height: 0.,
                    distance_to_border: 0x0205,
                },
            ]],
        }],
        unknown2: vec![],
    }
}

#[test]
fn mechanism_gate_restores_easing_then_opens_only_m09_group_two_and_fires_link_once() {
    let mut gate = node(33, 31, 15);
    gate.ladder_nav_coord1.0 = -2;
    gate.sce_proc_id = 500;
    let nodes = vec![gate, node(15, 23, u16::MAX)];
    let scene = scene_with_nav(nodes.clone(), "M09", gate_nav(), crate::GameType::PAL3);
    assert_eq!(scene.nav().get(0, 0, 0).unwrap().distance_to_border, 0);
    scene.trigger_mechanism(33);
    assert_eq!(scene.update_mechanisms(0.75), vec![500]);
    assert_eq!(scene.switch_state(15), 0);
    assert_eq!(scene.nav().get(0, 0, 0).unwrap().distance_to_border, 0);
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let saved: PersistentState =
        serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    let restored = scene_with_nav(nodes.clone(), "m09", gate_nav(), crate::GameType::PAL3);
    restored.restore_mechanisms(&saved);
    let height = restored
        .get_root_object(33)
        .unwrap()
        .transform()
        .borrow()
        .position()
        .y;
    assert!((height - (10. - 12. * (1. - std::f32::consts::FRAC_1_SQRT_2) * 0.5)).abs() < 0.0001);
    assert!(restored.update_mechanisms(2.25).is_empty());
    assert_eq!(restored.switch_state(15), 1);
    assert_eq!(restored.nav().get(0, 0, 0).unwrap().distance_to_border, 2);
    assert_eq!(
        restored.get_distance_to_border_by_scene_coord(0, &Vec3::new(1., 0., 1.)),
        2.
    );
    assert_eq!(restored.nav().get(0, 1, 0).unwrap().distance_to_border, 0);
    assert_eq!(restored.nav().get(0, 2, 0).unwrap().distance_to_border, 5);
    assert!(!restored.get_root_object(33).unwrap().visible());
    restored.update_mechanisms(0.);
    assert_eq!(restored.switch_state(15), 1);
    let mut saved = PersistentState::new("OpenPAL3".into());
    restored.snapshot_mechanisms(&mut saved);
    let reloaded = scene_with_nav(nodes.clone(), "m09", gate_nav(), crate::GameType::PAL3);
    reloaded.restore_mechanisms(&saved);
    assert_eq!(reloaded.nav().get(0, 0, 0).unwrap().distance_to_border, 2);
    reloaded.trigger_mechanism(33);
    assert!(reloaded.update_mechanisms(10.).is_empty());
    assert_eq!(reloaded.switch_state(15), 1);

    let other = scene_with_nav(nodes, "m01", gate_nav(), crate::GameType::PAL3);
    other.trigger_mechanism(33);
    other.update_mechanisms(3.);
    assert_eq!(other.nav().get(0, 0, 0).unwrap().distance_to_border, 0);
}

#[test]
fn mechanism_pal3a_retains_navigation_word_and_has_no_new_native_objects() {
    let scene = scene_with_nav(
        vec![node(2, 7, u16::MAX), node(33, 31, u16::MAX)],
        "m09",
        gate_nav(),
        crate::GameType::PAL3A,
    );
    scene.trigger_mechanism(2);
    scene.trigger_mechanism(33);
    assert!(scene.update_mechanisms(10.).is_empty());
    let mut saved = PersistentState::new("PAL3A".into());
    scene.snapshot_mechanisms(&mut saved);
    assert!(saved.mechanism_states("m09", "7").unwrap().is_empty());
    assert_eq!(scene.nav().get(0, 0, 0).unwrap().distance_to_border, 0x0200);
}

#[test]
fn mechanism_gate_link_invokes_target_activation_proc_in_the_completion_tick() {
    let mut first = node(1, 31, 2);
    first.ladder_nav_coord1.0 = 6;
    let mut target = node(2, 31, u16::MAX);
    target.sce_proc_id = 501;
    let scene = scene(vec![first, target]);
    scene.trigger_mechanism(1);
    assert_eq!(scene.update_mechanisms(1.), vec![501]);
    assert!(scene.update_mechanisms(0.).is_empty());
}

#[test]
fn mechanism_gate_retriggers_repeat_immediate_proc_but_not_its_movement_or_link() {
    let mut gate = node(1, 31, 2);
    gate.ladder_nav_coord1.0 = 6;
    gate.sce_proc_id = 500;
    let scene = scene(vec![gate, node(2, 23, u16::MAX)]);
    scene.trigger_mechanism(1);
    assert_eq!(scene.take_mechanism_procs(), vec![500]);
    scene.update_mechanisms(0.5);
    scene.trigger_mechanism(1);
    assert_eq!(scene.take_mechanism_procs(), vec![500]);
    scene.update_mechanisms(0.5);
    assert_eq!(scene.switch_state(2), 1);
    scene.update_mechanisms(0.);
    scene.update_mechanisms(2.);
    assert_eq!(scene.switch_state(2), 1);
}

fn bridge_scene() -> ScnScene {
    floor_scene(7)
}

fn floor_scene(kind: u16) -> ScnScene {
    let mut bridge = node(2, kind, u16::MAX);
    bridge.position = Vec3::new(150., 10., 150.);
    let mut nodes = vec![];
    if kind == 22 {
        bridge.ladder_nav_coord1.1 = 1;
        bridge.ladder_nav_coord2.0 = 9;
        nodes.push(node(9, 23, u16::MAX));
    }
    nodes.push(bridge);
    let nav = NavFile {
        version: 1,
        maps: (0..2)
            .map(|_| NavMap {
                layer_triggers: None,
                min_coord: Vec3::new(0., 0., 0.),
                max_coord: Vec3::new(320., 0., 320.),
                width: 15,
                height: 15,
                map: vec![
                    vec![
                        NavMapPoint {
                            height: 0.,
                            distance_to_border: 0,
                        };
                        15
                    ];
                    15
                ],
            })
            .collect(),
        unknown2: vec![],
    };
    let scene = scene_with_nav(nodes, "m19", nav, crate::GameType::PAL3);
    let entity = scene.get_root_object(2).unwrap();
    attach_bounds(
        &entity,
        Vec3::new(-40., 0., -100.),
        Vec3::new(40., 1., 100.),
    );
    scene
}

fn attach_bounds(entity: &crosscom::ComRc<radiance::comdef::IEntity>, min: Vec3, max: Vec3) {
    let mut vertices = VertexBuffer::new(VertexComponents::POSITION, 2);
    vertices.set_data(0, Some(&min), None, None, None);
    vertices.set_data(1, Some(&max), None, None, None);
    let mesh = StaticMeshComponent::new(
        entity.clone(),
        vec![Geometry {
            vertices,
            indices: vec![],
            material: MaterialDef::builder(ShaderProgram::TexturedNoLight).build(),
        }],
        Rc::new(UnusedFactory),
    );
    entity.add_component(
        IStaticMeshComponent::uuid(),
        crosscom::ComRc::from_object(mesh),
    );
}

#[test]
fn mechanism_bridge_floor_tracks_rotation_activity_and_save_restore_without_changing_other_layers()
{
    let scene = bridge_scene();
    scene.update_mechanisms(0.);
    assert_eq!(scene.get_height(0, (7., 12.)), 10.);
    assert_eq!(scene.nav().get(0, 7, 12).unwrap().distance_to_border, 2);
    assert_eq!(scene.nav().get(0, 12, 7).unwrap().distance_to_border, 0);
    assert_eq!(scene.nav().get(0, 8, 12).unwrap().distance_to_border, 0);
    assert_eq!(scene.nav().get(1, 7, 12).unwrap().distance_to_border, 0);
    scene.trigger_mechanism(2);
    assert_eq!(scene.nav().get(0, 7, 12).unwrap().distance_to_border, 0);
    scene.update_mechanisms(3.);
    assert_eq!(scene.nav().get(0, 7, 12).unwrap().distance_to_border, 0);
    assert_eq!(scene.nav().get(0, 12, 7).unwrap().distance_to_border, 2);
    assert_eq!(scene.nav().get(0, 12, 8).unwrap().distance_to_border, 2);
    assert_eq!(scene.nav().get(0, 12, 9).unwrap().distance_to_border, 0);
    assert_eq!(scene.get_height(0, (12., 7.)), 10.);
    assert_eq!(
        scene.get_distance_to_border_by_scene_coord(0, &Vec3::new(250., 0., 150.)),
        2.
    );
    assert!(scene.nav().check_connectivity(0, (10, 7), (13, 7)));
    assert!(!scene.nav().check_connectivity(1, (10, 7), (13, 7)));
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let restored = bridge_scene();
    restored.restore_mechanisms(&saved);
    assert_eq!(restored.nav().get(0, 12, 7).unwrap().distance_to_border, 2);
    restored.set_mechanism_active(2, false);
    assert_eq!(restored.nav().get(0, 12, 7).unwrap().distance_to_border, 0);
    restored.set_mechanism_active(2, true);
    assert_eq!(restored.nav().get(0, 12, 7).unwrap().distance_to_border, 2);
}

fn bind_player(
    scene: &ScnScene,
    position: Vec3,
    layer: usize,
    with_action: bool,
) -> Option<crosscom::ComRc<IAnimatedMeshComponent>> {
    use super::{RoleAnimationRepeatMode, RoleController};
    use crate::openpal3::comdef::IRoleController;
    let player = scene.get_role_entity(0).unwrap();
    player.transform().borrow_mut().set_position(&position);
    let factory = Rc::new(UnusedFactory);
    let assets = Rc::new(AssetManager::new(
        factory.clone(),
        Rc::new(mini_fs::MiniFs::new(true)),
    ));
    let clip = || {
        let clip = AnimatedMeshComponent::new(player.clone(), factory.clone());
        clip.set_morph_targets(
            vec![
                MorphTarget::new(vec![], 0., factory.clone()),
                MorphTarget::new(vec![], 1., factory.clone()),
            ],
            0.5,
        );
        crosscom::ComRc::<IAnimatedMeshComponent>::from_object(clip)
    };
    let controller =
        RoleController::new_from_idle_animation(player.clone(), assets, "test", "c01", clip());
    controller.set_nav_layer(layer);
    controller.set_active(true);
    let action = with_action.then(|| {
        let action = clip();
        controller.play_anim_mesh(
            "c10".into(),
            action.clone(),
            RoleAnimationRepeatMode::Repeat(1),
        );
        controller.idle();
        action
    });
    player.add_component(
        IRoleController::uuid(),
        crosscom::ComRc::from_object(controller),
    );
    scene.set_mechanism_player(Some((player, layer)));
    action
}

#[test]
fn mechanism_platform_carries_rider_and_switch_across_layers_and_restores_in_flight() {
    use super::RoleController;
    let scene = floor_scene(22);
    bind_player(&scene, Vec3::new(150., 10., 150.), 0, false);
    scene.trigger_mechanism(2);
    scene.update_mechanisms(0.5);
    let player = scene.get_role_entity(0).unwrap();
    assert_eq!(
        Vec3::sub(
            &player.transform().borrow().position(),
            &Vec3::new(150., 25., 150.)
        )
        .norm2(),
        0.
    );
    assert_eq!(
        scene
            .get_root_object(9)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y,
        25.
    );
    assert_eq!(scene.get_height(0, (7., 7.)), 25.);
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let saved: PersistentState =
        serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();

    let restored = floor_scene(22);
    bind_player(&restored, Vec3::new(150., 25., 150.), 0, false);
    restored.restore_mechanisms(&saved);
    restored.update_mechanisms(0.5);
    let player = restored.get_role_entity(0).unwrap();
    let controller = RoleController::get_role_controller(player.clone()).unwrap();
    assert_eq!(
        Vec3::sub(
            &player.transform().borrow().position(),
            &Vec3::new(150., 40., 150.)
        )
        .norm2(),
        0.
    );
    assert_eq!(controller.inner::<RoleController>().nav_layer(), 1);
    assert_eq!(restored.nav().get(0, 7, 7).unwrap().distance_to_border, 0);
    assert_eq!(restored.get_height(1, (7., 7.)), 40.);
    let mut upper = PersistentState::new("OpenPAL3".into());
    restored.snapshot_mechanisms(&mut upper);
    assert_eq!(
        upper.mechanism_states("m19", "7").unwrap()[&9].layer,
        Some(1)
    );
    restored.set_mechanism_player(Some((player.clone(), 1)));
    restored.trigger_mechanism(2);
    restored.update_mechanisms(1.);
    assert_eq!(
        Vec3::sub(
            &player.transform().borrow().position(),
            &Vec3::new(150., 10., 150.)
        )
        .norm2(),
        0.
    );
    assert_eq!(controller.inner::<RoleController>().nav_layer(), 0);
    let mut saved = PersistentState::new("OpenPAL3".into());
    restored.snapshot_mechanisms(&mut saved);
    let states = saved.mechanism_states("m19", "7").unwrap();
    assert_eq!(states[&9].layer, Some(0));
    assert_eq!(
        restored
            .get_root_object(9)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y,
        10.
    );
    assert_eq!(restored.get_height(0, (7., 7.)), 10.);
    assert_eq!(restored.nav().get(1, 7, 7).unwrap().distance_to_border, 0);
}

#[test]
fn mechanism_platform_boarding_uses_footprint_and_layer_and_does_not_recapture_mid_motion() {
    for (position, layer) in [
        (Vec3::new(190., 10., 150.), 0),
        (Vec3::new(150., 10., 150.), 1),
    ] {
        let scene = floor_scene(22);
        bind_player(&scene, position, layer, false);
        scene.trigger_mechanism(2);
        scene.update_mechanisms(0.5);
        let player = scene.get_role_entity(0).unwrap();
        assert_eq!(
            Vec3::sub(&player.transform().borrow().position(), &position).norm2(),
            0.
        );
        player
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(150., 10., 150.));
        scene.set_mechanism_player(Some((player.clone(), 0)));
        scene.update_mechanisms(0.5);
        assert_eq!(player.transform().borrow().position().y, 10.);
    }
}

#[test]
fn mechanism_manual_switch_approaches_then_waits_for_c10_before_linking() {
    use super::{RoleController, RoleState};
    let mut switch = node(9, 23, 3);
    switch.ladder_nav_coord1 = (0, 0);
    switch.remaining_triggers = 1;
    switch.sce_proc_id = 500;
    let scene = scene(vec![switch, node(3, 22, u16::MAX)]);
    attach_bounds(
        &scene.get_root_object(9).unwrap(),
        Vec3::new(-10., 0., -10.),
        Vec3::new(10., 10., 10.),
    );
    let action = bind_player(&scene, Vec3::new(150., 10., 0.), 0, true).unwrap();
    scene.trigger_mechanism(9);
    assert_eq!(scene.switch_state(9), 0);
    assert!(!scene.mechanism_input_locked());
    assert_eq!(
        scene.interact_mechanism(0, &Vec3::new(150., 10., 0.)),
        Some(None)
    );
    assert!(scene.mechanism_input_locked());
    assert_eq!(scene.switch_state(9), 0);
    assert!(scene.update_mechanisms(1.).is_empty());
    let player = scene.get_role_entity(0).unwrap();
    let controller = RoleController::get_role_controller(player.clone()).unwrap();
    assert_eq!(player.transform().borrow().position().x, 50.);
    assert_eq!(scene.switch_state(9), 0);
    scene.update_mechanisms(0.5);
    assert_eq!(player.transform().borrow().position().x, 10.);
    assert_eq!(scene.switch_state(9), 1);
    scene.update_mechanisms(0.);
    assert_eq!(
        controller.inner::<RoleController>().state(),
        RoleState::PlayingAnimation
    );
    assert!(scene.update_mechanisms(10.).is_empty());
    assert!(scene.mechanism_input_locked());
    assert_eq!(
        scene
            .get_root_object(3)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y,
        10.
    );
    let action = action.query_interface::<IComponent>().unwrap();
    action.on_updating(2.);
    action.on_updating(0.);
    controller
        .query_interface::<IComponent>()
        .unwrap()
        .on_updating(0.);
    assert_eq!(scene.update_mechanisms(0.), vec![500]);
    assert!(!scene.mechanism_input_locked());
    scene.update_mechanisms(0.5);
    assert_eq!(
        scene
            .get_root_object(3)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y,
        25.
    );
}

#[test]
fn mechanism_manual_switch_restores_action_without_retoggling_and_missing_clip_cannot_lock_input() {
    let mut switch = node(9, 23, u16::MAX);
    switch.ladder_nav_coord1 = (0, 0);
    switch.remaining_triggers = 1;
    switch.sce_proc_id = 500;
    let scene = scene(vec![switch.clone()]);
    bind_player(&scene, Vec3::new(10., 10., 0.), 0, true);
    scene.interact_mechanism(0, &Vec3::new(10., 10., 0.));
    scene.update_mechanisms(0.);
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let saved: PersistentState =
        serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    let replayed = self::scene(vec![switch.clone()]);
    bind_player(&replayed, Vec3::new(10., 10., 0.), 0, true);
    replayed.restore_mechanisms(&saved);
    assert!(replayed.update_mechanisms(0.).is_empty());
    assert!(replayed.update_mechanisms(10.).is_empty());
    assert!(replayed.mechanism_input_locked());
    assert_eq!(replayed.switch_state(9), 1);
    let restored = self::scene(vec![switch]);
    bind_player(&restored, Vec3::new(10., 10., 0.), 0, false);
    restored.restore_mechanisms(&saved);
    assert!(restored.mechanism_input_locked());
    assert_eq!(restored.switch_state(9), 1);
    assert_eq!(restored.update_mechanisms(0.), vec![500]);
    assert!(!restored.mechanism_input_locked());
    assert_eq!(restored.switch_state(9), 1);
    assert!(restored.update_mechanisms(1.).is_empty());
}
