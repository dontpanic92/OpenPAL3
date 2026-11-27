use super::ScnScene;
use super::mechanism::{ChestPhase, ChestReward, ChestState, Mechanism};
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

#[test]
fn deferred_scene_loading_installs_triggers_only_when_their_work_completes() {
    let mut item = node(12, 11, u16::MAX);
    item.sce_proc_id = 500;
    let scene = scene(vec![item]);
    let position = Vec3::new(0., 10., 0.);
    cache_player(&scene, position);
    scene.defer_loading();
    radiance::comdef::IComponentImpl::on_loading(&scene);
    assert_eq!(scene.test_item_trigger(&position), None);
    assert_eq!(scene.advance_loading(), 0.8);
    assert_eq!(scene.test_item_trigger(&position), None);
    assert_eq!(scene.advance_loading(), 0.85);
    assert_eq!(scene.test_item_trigger(&position), Some(500));
}

#[test]
#[ignore = "requires original game assets via PAL3_ASSET_PATH"]
fn shipped_treasure_mouse_search_uses_authored_order_without_claiming_or_moving_items() {
    use radiance::comdef::ISceneExt;

    let path = std::env::var("PAL3_ASSET_PATH").expect("PAL3_ASSET_PATH");
    let assets = Rc::new(AssetManager::new(
        Rc::new(UnusedFactory),
        Rc::new(packfs::init_virtual_fs(std::path::Path::new(&path), None)),
    ));
    let mut inactive = node(7, 24, 0);
    inactive.active = 0;
    inactive.ladder_nav_coord1.1 = 1;
    let ordinary = node(8, 24, 0);
    let mut first = node(50, 24, 0);
    first.ladder_nav_coord1 = (6201, 1);
    first.position = Vec3::new(200., 10., 0.);
    let mut near = node(2, 24, 0);
    near.ladder_nav_coord1 = (6112, 1);
    near.position = Vec3::new(20., 10., 0.);
    let nodes = vec![inactive, ordinary, first, near];
    let core = CoreScene::create();
    for node in &nodes {
        let entity = CoreEntity::create(format!("OBJECT_{}", node.index), node.active != 0);
        entity.transform().borrow_mut().set_position(&node.position);
        core.add_entity(entity);
    }
    let scene = ScnScene::new(
        core.clone(),
        &assets,
        "m01",
        "1",
        ScnFile {
            cpk_name: "m01".into(),
            scn_name: "1".into(),
            scn_base_name: "1".into(),
            scene_type: 2,
            is_night: false,
            skybox_id: 0,
            roles: vec![],
            nodes,
        },
        NavFile {
            version: 1,
            maps: vec![NavMap {
                layer_triggers: None,
                min_coord: Vec3::new(0., 0., 0.),
                max_coord: Vec3::new(240., 0., 12.),
                width: 20,
                height: 1,
                map: vec![vec![
                    NavMapPoint {
                        height: 100.,
                        distance_to_border: 0x0200
                    };
                    20
                ]],
            }],
            unknown2: vec![],
        },
    );
    let eye = Vec3::new(400., 410., 400.);
    core.camera_mut().transform_mut().set_position(&eye);
    assert!(scene.prepare_treasure_mouse());
    assert!(scene.prepare_treasure_mouse());
    let mice: Vec<_> = core
        .entities()
        .into_iter()
        .filter(|entity| entity.name() == "TREASURE_MOUSE")
        .collect();
    assert_eq!(mice.len(), 1);
    let mouse = &mice[0];
    mouse.load();
    assert!(
        mouse
            .get_component(IAnimatedMeshComponent::uuid())
            .is_some()
    );
    assert!(mouse.get_rendering_component().is_some());
    let mut saved = PersistentState::new("test".into());
    scene.snapshot_mechanisms(&mut saved);
    let before = serde_json::to_value(&saved).unwrap();
    scene.launch_treasure_mouse(Vec3::new(0., 10., 0.));
    assert_eq!(mouse.transform().borrow().position().x, 200.);
    assert_eq!(mouse.transform().borrow().position().y, 101.);
    scene.update_treasure_mouse(1.);
    assert!((core.camera().transform().position().x - 429.2893).abs() < 0.0001);
    assert_eq!(mouse.transform().borrow().position().x, 200.);
    assert!(mouse.visible());
    scene.update_treasure_mouse(3.);
    assert!(!mouse.visible());
    assert!(!scene.treasure_mouse_active());
    assert_eq!(core.camera().transform().position().x, eye.x);
    assert!(scene.take_pickup_rewards().is_empty());
    scene.snapshot_mechanisms(&mut saved);
    assert_eq!(serde_json::to_value(&saved).unwrap(), before);
    mouse.unload();
}

#[test]
fn mouse_search_handles_empty_scenes_and_missing_target_navigation() {
    let empty = scene(vec![node(7, 24, 0)]);
    assert!(empty.prepare_treasure_mouse());
    empty.launch_treasure_mouse(Vec3::new(0., 10., 0.));
    assert!(!empty.treasure_mouse_active());
    assert_eq!(empty.take_mechanism_notices(), vec!["没有找到宝物"]);
    assert!(empty.take_pickup_rewards().is_empty());
    let mut target = node(7, 24, 0);
    target.ladder_nav_coord1.1 = 1;
    target.nav_layer = 1;
    let invalid = scene(vec![target]);
    assert!(!invalid.prepare_treasure_mouse());
    invalid.launch_treasure_mouse(Vec3::new(0., 10., 0.));
    assert!(!invalid.treasure_mouse_active());
    assert!(invalid.take_mechanism_notices().is_empty());
}

#[test]
#[ignore = "requires original game assets via PAL3_ASSET_PATH"]
fn shipped_bee_loads_its_actor_and_flies_to_centered_masked_nav_coordinates() {
    use crate::openpal3::comdef::IScnSceneComponent;
    use radiance::comdef::ISceneExt;

    let path = std::env::var("PAL3_ASSET_PATH").expect("PAL3_ASSET_PATH");
    let assets = Rc::new(AssetManager::new(
        Rc::new(UnusedFactory),
        Rc::new(packfs::init_virtual_fs(std::path::Path::new(&path), None)),
    ));
    let core = assets.load_scn("m01", "1");
    let component = core
        .get_component(IScnSceneComponent::uuid())
        .unwrap()
        .query_interface::<IScnSceneComponent>()
        .unwrap();
    let scene = component.inner::<ScnScene>();
    assert!(scene.prepare_bee());
    assert!(scene.prepare_bee());
    let bees: Vec<_> = core
        .entities()
        .into_iter()
        .filter(|e| e.name() == "GUIDE_BEE")
        .collect();
    assert_eq!(bees.len(), 1);
    let bee = &bees[0];
    bee.load();
    assert!(bee.get_component(IAnimatedMeshComponent::uuid()).is_some());
    assert!(bee.get_rendering_component().is_some());
    assert!(!bee.visible());
    let leader = CoreEntity::create("ROLE_0".into(), true);
    let origin = Vec3::new(500., 10., -1100.);
    leader.transform().borrow_mut().set_position(&origin);
    assert!(!scene.launch_bee(origin, 3, 3, 5));
    assert!(scene.launch_bee(origin, 2, 3, 5));
    assert_eq!(bee.transform().borrow().position().x, origin.x);
    scene.update_mechanisms(1000.);
    let position = bee.transform().borrow().position();
    let min = scene.nav_min_coord(0);
    assert_eq!(position.x, min.x + 42.);
    assert_eq!(position.z, min.z + 66.);
    assert_eq!(
        position.y,
        scene.nav().get(0, 3, 5).map_or(0., |p| p.height) + 80.
    );
    assert!(bee.visible());
    assert_eq!(
        Vec3::sub(&leader.transform().borrow().position(), &origin).norm2(),
        0.
    );
    scene.update_mechanisms(0.001);
    assert!(!bee.visible());
    bee.update(0.1);
    assert!(!bee.visible());
    assert!(scene.launch_bee(origin, 2, -1, -1));
    scene.update_mechanisms(1000.);
    let position = bee.transform().borrow().position();
    assert_eq!(position.x, min.x - 6.);
    assert_eq!(position.z, min.z - 6.);
    assert_eq!(position.y, 80.);
    let reloaded = assets.load_scn("m01", "1");
    let reloaded = reloaded.get_component(IScnSceneComponent::uuid()).unwrap();
    assert!(reloaded.inner::<ScnScene>().prepare_bee());
    bee.unload();
}

#[test]
#[ignore = "requires original game assets via PAL3_ASSET_PATH"]
fn shipped_item_previews_load_variant_textures_and_reject_missing_assets() {
    use crate::openpal3::loaders::pol::create_menu_preview;
    let path = std::env::var("PAL3_ASSET_PATH").expect("PAL3_ASSET_PATH");
    let assets = AssetManager::new(
        Rc::new(UnusedFactory),
        Rc::new(packfs::init_virtual_fs(std::path::Path::new(&path), None)),
    );
    let mut variants = vec![];
    for (model, image) in [("MB", "6"), ("MB", "7"), ("PE", "2"), ("WXA", "4")] {
        let folder = format!("/basedata/basedata/Item/{model}");
        let texture = format!("{folder}/{model}{image}.tga");
        let entity = create_menu_preview(
            &assets.component_factory(),
            assets.vfs(),
            &format!("{folder}/{model}.pol"),
            Some(&texture),
        )
        .unwrap();
        let component = entity
            .get_component(IStaticMeshComponent::uuid())
            .unwrap()
            .query_interface::<IStaticMeshComponent>()
            .unwrap();
        let geometries = component.inner::<StaticMeshComponent>().get_geometries();
        assert!(!geometries.is_empty());
        for geometry in &*geometries {
            assert!(!geometry.indices.is_empty());
            assert_eq!(
                geometry.material.textures()[0].name(),
                format!("{folder}/{model}{image}.dds")
            );
        }
        variants.push(
            geometries[0].material.textures()[0]
                .with_image(|image| image.expect("shipped texture decodes").as_raw().clone()),
        );
        drop(geometries);
        entity.load();
        entity.unload();
        assert!(entity.get_component(IStaticMeshComponent::uuid()).is_none());
    }
    assert_ne!(variants[0], variants[1]);
    assert!(
        create_menu_preview(
            &assets.component_factory(),
            assets.vfs(),
            "/missing.pol",
            Some("/missing.tga")
        )
        .is_err()
    );
    assert!(
        create_menu_preview(
            &assets.component_factory(),
            assets.vfs(),
            "/basedata/basedata/Item/MB/MB.pol",
            Some("/missing.tga")
        )
        .is_err()
    );
    for model in ["Lineup/wl.pol", "SWORD/Data/Sword.pol"] {
        let entity = create_menu_preview(
            &assets.component_factory(),
            assets.vfs(),
            &format!("/basedata/basedata/ui/GameMainUI/{model}"),
            None,
        )
        .unwrap();
        let component = entity.get_component(IStaticMeshComponent::uuid()).unwrap();
        let geometries = component.inner::<StaticMeshComponent>().get_geometries();
        assert!(!geometries.is_empty());
        for geometry in geometries.iter() {
            assert!(!geometry.indices.is_empty());
            assert!(
                geometry.material.textures()[0].with_image(|i| i.is_some()),
                "native UI model materials must load their own shipped textures"
            );
        }
    }
}

#[test]
#[ignore = "requires original game assets via PAL3_ASSET_PATH"]
fn shipped_cvd_loops_switches_and_bridge_deform_and_restore_both_render_passes() {
    fn bounds(entity: &crosscom::ComRc<radiance::comdef::IEntity>) -> Vec<([f32; 3], [f32; 3])> {
        let mut result = vec![];
        if let Some(component) = entity.get_rendering_component() {
            result.extend(
                component
                    .render_objects()
                    .iter()
                    .filter_map(|object| object.as_dyn().local_aabb()),
            );
        }
        for child in entity.children() {
            result.extend(bounds(&child));
        }
        result
    }

    let path = std::env::var("PAL3_ASSET_PATH").expect("PAL3_ASSET_PATH");
    let vfs = Rc::new(packfs::init_virtual_fs(std::path::Path::new(&path), None));
    let assets = AssetManager::new(Rc::new(UnusedFactory), vfs.clone());
    for model in ["wz05.cvd", "wz06.cvd", "_e.cvd"] {
        let load = || {
            if model.starts_with('_') {
                assets.load_scn_cvd("m01", "1", model, false, 1)
            } else {
                assets.load_object_item_cvd(model, 1, true)
            }
            .unwrap()
        };
        let entity = load();
        if model == "wz06.cvd" {
            super::cvd_entity::configure_scene_playback(&entity, 29, 0);
        }
        entity.load();
        let duration = super::cvd_entity::animation_duration(&entity);
        assert!(duration > 0., "{model}");
        let initial = bounds(&entity);
        assert!(!initial.is_empty());
        entity.update(duration * 0.25);
        assert_eq!(bounds(&entity), initial, "{model} stays externally owned");
        if !model.starts_with('_') {
            assert_eq!(initial.len(), 6, "three materials, each with two passes");
            for pair in initial.chunks_exact(2) {
                assert_eq!(pair[0], pair[1]);
            }
        }
        super::cvd_entity::set_animation_time(&entity, duration * 0.25);
        assert_ne!(
            bounds(&entity),
            initial,
            "{model} must deform, not just move nodes"
        );
        super::cvd_entity::set_animation_time(&entity, duration);
        let final_pose = bounds(&entity);
        entity.update(duration * 0.5);
        assert_eq!(
            bounds(&entity),
            final_pose,
            "{model} keeps its completed pose"
        );
        super::cvd_entity::set_animation_time(&entity, duration + 1.);
        assert_eq!(bounds(&entity), final_pose, "{model} must clamp at the end");
        let restored = load();
        super::cvd_entity::configure_scene_playback(&restored, 0, 0);
        super::cvd_entity::set_animation_time(&restored, duration);
        restored.load();
        restored.update(duration * 0.5);
        assert_eq!(
            bounds(&restored),
            final_pose,
            "{model} restores before loading"
        );
        entity.unload();
        restored.unload();
    }

    // Q01/b node 42 is a native type-17 ambient model with mode 0.
    for game in [crate::GameType::PAL3, crate::GameType::PAL3A] {
        let assets = Rc::new(AssetManager::new_for_game(
            Rc::new(UnusedFactory),
            vfs.clone(),
            game,
        ));
        let mut fixture = node(42, 17, u16::MAX);
        fixture.name = "_bn03a.cvd".into();
        fixture.ladder_nav_coord1.1 = 0;
        let scene = ScnScene::new(
            CoreScene::create(),
            &assets,
            "q01",
            "b",
            ScnFile {
                cpk_name: "q01".into(),
                scn_name: "b".into(),
                scn_base_name: "b".into(),
                scene_type: 0,
                is_night: false,
                skybox_id: 0,
                roles: vec![],
                nodes: vec![fixture],
            },
            NavFile {
                version: 1,
                maps: vec![],
                unknown2: vec![],
            },
        );
        scene.load_objects_for_test();
        let idle = scene.get_root_object(42).unwrap();
        idle.load();
        let initial = bounds(&idle);
        assert!(!initial.is_empty());
        let duration = super::cvd_entity::animation_duration(&idle);
        idle.update(duration * 0.25);
        let animated = bounds(&idle);
        assert_ne!(
            animated, initial,
            "both games' scene loading enables ambient mesh playback"
        );
        super::cvd_entity::set_animation_time(&idle, duration * 0.25);
        idle.update(duration * 0.5);
        assert_eq!(
            bounds(&idle),
            animated,
            "explicit ownership stops ambient playback"
        );
        idle.unload();
    }
}

#[test]
#[ignore = "requires original game assets via PAL3_ASSET_PATH"]
fn shipped_chest_uses_real_animation_duration_and_broken_model() {
    use crate::openpal3::comdef::IScnSceneComponent;
    let path = std::env::var("PAL3_ASSET_PATH").expect("PAL3_ASSET_PATH");
    let assets = Rc::new(AssetManager::new(
        Rc::new(UnusedFactory),
        Rc::new(packfs::init_virtual_fs(std::path::Path::new(&path), None)),
    ));
    let root = assets.load_scn("m01", "1");
    let component = root.get_component(IScnSceneComponent::uuid()).unwrap();
    component.on_loading();
    let component = component.query_interface::<IScnSceneComponent>().unwrap();
    let scene = component.inner::<ScnScene>();
    let original = scene.get_root_object(14).unwrap();
    let duration = super::cvd_entity::animation_duration(&original);
    assert!(duration > 0.);
    for attempts in [0, 3] {
        let mut saved = PersistentState::new("OpenPAL3".into());
        scene.snapshot_mechanisms(&mut saved);
        let mut states = saved.mechanism_states("m01", "1").unwrap().clone();
        let chest = states.get_mut(&14).unwrap().chest.as_mut().unwrap();
        chest.attempts = attempts;
        chest.phase = ChestPhase::Opening {
            role: 1,
            elapsed: 0.,
        };
        saved.set_mechanism_states("m01", "1", states);
        scene.restore_mechanisms(&saved);
        let entity = scene.get_root_object(14).unwrap();
        let duration = super::cvd_entity::animation_duration(&entity);
        if attempts == 3 {
            assert!(
                !std::ptr::eq(&*entity, &*original),
                "breaking must replace the actual CVD, not recolor it"
            );
            let expected = assets.load_object_item_cvd("wz20_5.cvd", 14, true).unwrap();
            assert_eq!(duration, super::cvd_entity::animation_duration(&expected));
        }
        if duration > 0. {
            scene.update_mechanisms(duration * 0.5);
            assert!(scene.take_chest_rewards().is_empty());
            assert!(scene.take_pickup_rewards().is_empty());
        }
        scene.update_mechanisms(duration * 0.5 + 0.01);
        assert_eq!(
            scene.take_pickup_rewards(),
            if attempts == 0 {
                vec![(82111, 0), (82202, 0), (84102, 0), (83102, 0), (6201, 0)]
            } else {
                vec![]
            }
        );
        assert_eq!(
            scene.take_chest_rewards(),
            vec![ChestReward {
                role: 1,
                experience: if attempts == 0 { 55 } else { 0 },
                favor: if attempts == 3 { -2 } else { 0 },
            }]
        );
    }
}

#[test]
#[ignore = "requires original game assets via PAL3_ASSET_PATH"]
fn shipped_deferred_scene_loading_matches_synchronous_entities_and_finishes_once() {
    use crate::openpal3::comdef::IScnSceneComponent;
    use radiance::comdef::{IScene, ISceneExt};
    let path = std::env::var("PAL3_ASSET_PATH").expect("PAL3_ASSET_PATH");
    let assets = Rc::new(AssetManager::new(
        Rc::new(UnusedFactory),
        Rc::new(packfs::init_virtual_fs(std::path::Path::new(&path), None)),
    ));
    let entities = |root: &crosscom::ComRc<IScene>| {
        root.root_entities()
            .iter()
            .map(|entity| {
                let p = entity.transform().borrow().position();
                (
                    entity.name(),
                    entity.visible(),
                    [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()],
                )
            })
            .collect::<Vec<_>>()
    };
    for (name, block) in [("m01", "1"), ("m24", "3")] {
        let synchronous = assets.load_scn(name, block);
        let sync_component = synchronous
            .get_component(IScnSceneComponent::uuid())
            .unwrap();
        sync_component.on_loading();
        let deferred = assets.load_scn_deferred(name, block);
        let component = deferred.get_component(IScnSceneComponent::uuid()).unwrap();
        // Keep the roots unloaded so this asset-only test needs no GPU.
        component.on_loading();
        assert!(deferred.root_entities().is_empty());
        let component = component.query_interface::<IScnSceneComponent>().unwrap();
        let scene = component.inner::<ScnScene>();
        let mut previous = 0.;
        let mut finished = false;
        for _ in 0..1000 {
            let progress = scene.advance_loading();
            assert!(progress >= previous && progress <= 1.);
            previous = progress;
            if progress == 1. {
                finished = true;
                break;
            }
        }
        assert!(
            finished,
            "{name}/{block} must finish without waiting for UI easing"
        );
        assert_eq!(
            entities(&deferred),
            entities(&synchronous),
            "{name}/{block}"
        );
        assert_eq!(scene.advance_loading(), 1.);
        assert_eq!(
            entities(&deferred),
            entities(&synchronous),
            "completion must not reload entities"
        );
    }
}

#[test]
#[ignore = "requires original game assets via PAL3_ASSET_PATH"]
fn shipped_plate_bridge_geometry() {
    use crate::openpal3::comdef::IScnSceneComponent;
    use radiance::comdef::IScene;
    let path = std::env::var("PAL3_ASSET_PATH").expect("PAL3_ASSET_PATH");
    let assets = Rc::new(AssetManager::new(
        Rc::new(UnusedFactory),
        Rc::new(packfs::init_virtual_fs(std::path::Path::new(&path), None)),
    ));
    for (name, block, pairs) in [
        ("m01", "1", &[(10, 11)][..]),
        ("m10", "3", &[(3, 4), (5, 6), (7, 8), (9, 10)][..]),
    ] {
        let root: crosscom::ComRc<IScene> = assets.load_scn(name, block);
        let component = root
            .get_component(IScnSceneComponent::uuid())
            .unwrap()
            .query_interface::<IScnSceneComponent>()
            .unwrap();
        let scene = component.inner::<ScnScene>();
        scene.load_objects_for_test();
        scene.set_mechanism_party_count(4);
        for &(plate, bridge) in pairs {
            let position = scene
                .get_root_object(plate)
                .unwrap()
                .transform()
                .borrow()
                .position();
            let player = CoreEntity::create("test-player".into(), true);
            player.transform().borrow_mut().set_position(&position);
            scene.set_mechanism_player(Some((player, 0)));
            scene.update_mechanisms(0.);
            assert_eq!(
                scene.interact_mechanism(0, &position),
                Some(None),
                "{name}/{block} plate{plate}"
            );
            for _ in 0..600 {
                scene.update_mechanisms(1. / 60.);
            }
            assert!(
                !scene.mechanism_input_locked(),
                "{name}/{block} bridge{bridge}"
            );
            let mut saved = PersistentState::new("OpenPAL3".into());
            scene.snapshot_mechanisms(&mut saved);
            let state = &saved.mechanism_states(name, block).unwrap()[&(bridge as u16)];
            assert!(state.action.as_ref().unwrap().animation_complete);
            let entity = scene.get_root_object(bridge).unwrap();
            let bounds = super::cvd_entity::model_bounds(&entity).unwrap();
            assert!(bounds.0.x.is_finite() && bounds.1.z.is_finite());
            let start = scene.scene_coord_to_nav_coord(0, &position);
            let end = scene.scene_coord_to_nav_coord(0, &entity.transform().borrow().position());
            let start = (start.0 as i32, start.1 as i32);
            let end = (end.0 as i32, end.1 as i32);
            let mut todo = std::collections::VecDeque::from([start]);
            let mut seen = std::collections::HashSet::from([start]);
            while let Some(p) = todo.pop_front() {
                if p == end {
                    break;
                }
                for (dx, dz) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
                    let q = (p.0 + dx, p.1 + dz);
                    if seen.contains(&q) {
                        continue;
                    }
                    if scene
                        .nav()
                        .get(0, q.0, q.1)
                        .is_some_and(|p| p.distance_to_border > 0)
                    {
                        seen.insert(q);
                        todo.push_back(q);
                    }
                }
            }
            assert!(
                seen.contains(&end),
                "{name}/{block} bridge{bridge} must connect to its plate's bank"
            );
        }
    }
}

#[test]
#[ignore = "requires original game assets via PAL3_ASSET_PATH"]
fn shipped_hazard_geometry_hits_grounded_actor_and_rejects_distant_actor() {
    use crate::openpal3::comdef::IScnSceneComponent;
    use common::store_ext::StoreExt2;
    let path = std::env::var("PAL3_ASSET_PATH").expect("PAL3_ASSET_PATH");
    let assets = Rc::new(AssetManager::new(
        Rc::new(UnusedFactory),
        Rc::new(packfs::init_virtual_fs(std::path::Path::new(&path), None)),
    ));
    let model = fileformats::mv3::read_mv3(&mut std::io::Cursor::new(
        assets
            .vfs()
            .read_to_end(assets.mv3_path("101", "C01"))
            .unwrap(),
    ))
    .unwrap();
    let model = &model.models[0];
    let actor = CoreEntity::create("test-player".into(), true);
    attach_bounds(
        &actor,
        Vec3::new(-model.aabb_max[0], model.aabb_min[1], -model.aabb_max[2]),
        Vec3::new(-model.aabb_min[0], model.aabb_max[1], -model.aabb_min[2]),
    );
    for (block, ids, fraction) in [
        ("1", &[6, 7, 8, 9][..], 0.1),
        ("2", &[5, 6, 7, 8, 9][..], 0.15),
    ] {
        let root = assets.load_scn("m24", block);
        let component = root
            .get_component(IScnSceneComponent::uuid())
            .unwrap()
            .query_interface::<IScnSceneComponent>()
            .unwrap();
        let scene = component.inner::<ScnScene>();
        scene.load_objects_for_test();
        scene.set_mechanism_player(Some((actor.clone(), 0)));
        for &id in ids {
            for &other in ids {
                scene.set_mechanism_active(other, other == id);
            }
            if block == "2" {
                scene.trigger_mechanism(id);
            }
            let mut position = scene
                .get_root_object(id as i32)
                .unwrap()
                .transform()
                .borrow()
                .position();
            let coord = scene.scene_coord_to_nav_coord(0, &position);
            position.y = scene.get_height(0, coord);
            actor.transform().borrow_mut().set_position(&position);
            let mut hit = false;
            for _ in 0..400 {
                scene.update_mechanisms(1. / 60.);
                if let Some(actual) = scene.update_hazard_damage(0., &mut 2.) {
                    assert_eq!(actual, fraction);
                    hit = true;
                    break;
                }
            }
            assert!(hit, "m24/{block} hazard{id} must hit a grounded actor");
            position.x += 10000.;
            actor.transform().borrow_mut().set_position(&position);
            assert_eq!(scene.update_hazard_damage(0., &mut 2.), None);
        }
    }
}

#[test]
#[ignore = "requires original game assets via PAL3_ASSET_PATH"]
fn shipped_mover_removes_and_rebuilds_its_navigation_footprint() {
    use crate::openpal3::comdef::IScnSceneComponent;
    let path = std::env::var("PAL3_ASSET_PATH").expect("PAL3_ASSET_PATH");
    let assets = Rc::new(AssetManager::new(
        Rc::new(UnusedFactory),
        Rc::new(packfs::init_virtual_fs(std::path::Path::new(&path), None)),
    ));
    let root = assets.load_scn("m24", "3");
    let component = root
        .get_component(IScnSceneComponent::uuid())
        .unwrap()
        .query_interface::<IScnSceneComponent>()
        .unwrap();
    let scene = component.inner::<ScnScene>();
    scene.load_objects_for_test();
    scene.update_mechanisms(0.);
    let navigation = || {
        let (width, height) = scene.nav().get_map_size(0);
        (0..height)
            .flat_map(|z| (0..width).map(move |x| (x, z)))
            .map(|(x, z)| {
                let point = scene.nav().get(0, x as i32, z as i32).unwrap();
                (point.height.to_bits(), point.distance_to_border)
            })
            .collect::<Vec<_>>()
    };
    let before = navigation();
    scene.trigger_mechanism(17);
    let moving = navigation();
    assert_ne!(before, moving, "moving object17 must release its floor");
    scene.update_mechanisms(4.);
    scene.update_mechanisms(4.);
    let after = navigation();
    assert_ne!(after, moving, "finished linked movers must rebuild floors");
    assert_ne!(after, before, "floors must follow their translated models");
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    scene.restore_mechanisms(&saved);
    assert_eq!(navigation(), after);
}

#[test]
fn native_hazard_collision_uses_pol_header_bounds_not_rendered_vertices() {
    let mut hammer = node(2, 28, u16::MAX);
    hammer.ladder_nav_coord1.0 = 1;

    let mut data = b"POLY".to_vec();
    for value in [100u32, 1] {
        data.extend(value.to_le_bytes());
    }
    data.extend([0; 52]);
    for value in [-1f32, 0., -1., 1., 100., 1.] {
        data.extend(value.to_le_bytes());
    }
    for value in [17u32, 3] {
        data.extend(value.to_le_bytes());
    }
    for vertex in [
        [-0.5f32, 0., -0.5, 0., 0.],
        [0.5, 0., -0.5, 0., 0.],
        [0., 1., 0.5, 0., 0.],
    ] {
        for value in vertex {
            data.extend(value.to_le_bytes());
        }
    }
    data.extend(1u32.to_le_bytes());
    data.extend([0; 72]);
    data.extend(1u32.to_le_bytes());
    let mut texture = [0; 64];
    texture[..10].copy_from_slice(b"unused.tga");
    data.extend(texture);
    data.extend([0; 12]);
    data.extend(1u32.to_le_bytes());
    for index in [0u16, 1, 2] {
        data.extend(index.to_le_bytes());
    }

    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let folder =
        std::env::temp_dir().join(format!("yaobow-pol-bounds-{}-{unique}", std::process::id()));
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("test.pol"), data).unwrap();
    let vfs = packfs::init_virtual_fs(&folder, None);
    let factory: Rc<dyn ComponentFactory> = Rc::new(UnusedFactory);
    let entity = crate::openpal3::loaders::pol::create_entity_from_pol_model(
        &factory,
        &vfs,
        "/test.pol",
        "OBJECT_2".into(),
        true,
    );
    std::fs::remove_file(folder.join("test.pol")).unwrap();
    std::fs::remove_dir(folder).unwrap();
    let core = CoreScene::create();
    core.add_entity(CoreEntity::create("ROLE_0".into(), true));
    core.add_entity(entity);
    let assets = Rc::new(AssetManager::new(factory, Rc::new(vfs)));
    let scene = ScnScene::new(
        core,
        &assets,
        "m17",
        "7",
        ScnFile {
            cpk_name: "m17".into(),
            scn_name: "7".into(),
            scn_base_name: "7".into(),
            scene_type: 0,
            is_night: false,
            skybox_id: 0,
            roles: vec![],
            nodes: vec![hammer],
        },
        NavFile {
            version: 1,
            maps: vec![],
            unknown2: vec![],
        },
    );
    cache_player(&scene, Vec3::new(0., 60., 0.));
    attach_bounds(
        &scene.get_role_entity(0).unwrap(),
        Vec3::new(-0.1, -0.1, -0.1),
        Vec3::new(0.1, 0.1, 0.1),
    );
    scene.trigger_mechanism(2);
    assert_eq!(scene.update_hazard_damage(0., &mut 2.), Some(0.15));
    cache_player(&scene, Vec3::new(0., 120., 0.));
    assert_eq!(scene.update_hazard_damage(0., &mut 2.), None);
}

#[test]
fn native_hazard_pitch_changes_collision_without_rotating_its_drawn_pose() {
    let mut hazard = node(2, 28, u16::MAX);
    hazard.position = Vec3::new(100., 123., 200.);
    hazard.rotation = 35.;
    hazard.ladder_nav_coord1.0 = 0;
    hazard.dw184 = vec![(-23f32).to_bits()];
    let scene = scene(vec![hazard]);
    // Original 0x429ee0 + gbOBBox::IsColOBB: hit with pitch, miss without.
    cache_player(&scene, Vec3::new(308.2461, 202.3186, 14.4087));
    attach_bounds(
        &scene.get_role_entity(0).unwrap(),
        Vec3::new(-0.1, -0.1, -0.1),
        Vec3::new(0.1, 0.1, 0.1),
    );
    let entity = scene.get_root_object(2).unwrap();
    attach_bounds(&entity, Vec3::new(-100., -3., -4.), Vec3::new(300., 5., 6.));
    scene.update_mechanisms((17f32 / 24.).asin());
    let before = entity.transform().borrow().matrix().clone();
    assert_eq!(
        before[1][2], 0.,
        "native swinging-hazard drawing omits pitch"
    );
    assert_eq!(scene.update_hazard_damage(0., &mut 2.), Some(0.1));
    assert_eq!(entity.transform().borrow().matrix()[1][2], before[1][2]);
}

#[test]
#[ignore = "requires original game assets via PAL3_ASSET_PATH"]
fn shipped_pickup_roots_use_authored_pitch() {
    use crate::openpal3::comdef::IScnSceneComponent;
    let path = std::env::var("PAL3_ASSET_PATH").expect("PAL3_ASSET_PATH");
    let assets = Rc::new(AssetManager::new(
        Rc::new(UnusedFactory),
        Rc::new(packfs::init_virtual_fs(std::path::Path::new(&path), None)),
    ));
    let root = assets.load_scn("m05", "2");
    let component = root
        .get_component(IScnSceneComponent::uuid())
        .unwrap()
        .query_interface::<IScnSceneComponent>()
        .unwrap();
    let scene = component.inner::<ScnScene>();
    scene.load_objects_for_test();
    // Native matrix columns from original gbengine rotation routines.
    for (id, expected) in [
        (7, [0.038276095, 0.9986295, 0.035693035]),
        (12, [0.017899951, 0.9986295, -0.04917971]),
    ] {
        let entity = scene.get_root_object(id).unwrap();
        let transform = entity.transform();
        let transform = transform.borrow();
        for axis in 0..3 {
            assert!((transform.matrix()[axis][1] - expected[axis]).abs() < 0.00001);
        }
    }
}

#[test]
fn plate_checks_party_and_horizontal_range_then_links_once_and_restores_lowered_pose() {
    let mut plate = node(10, 29, 11);
    plate.position.y = 123.;
    plate.ladder_nav_coord1.0 = 3;
    plate.sce_proc_id = 506;
    let nodes = vec![plate, node(11, 19, u16::MAX)];
    let scene = scene(nodes.clone());
    cache_player(&scene, Vec3::new(0., 123., 0.));
    scene.set_mechanism_party_count(2);
    assert_eq!(
        scene.interact_mechanism(0, &Vec3::new(0., 123., 0.)),
        Some(None)
    );
    assert_eq!(scene.take_mechanism_notices(), vec!["重量不够"]);
    assert!(!scene.mechanism_input_locked());
    scene.set_mechanism_party_count(3);
    assert_eq!(
        scene.interact_mechanism(0, &Vec3::new(36.01, 123., 0.)),
        None
    );
    assert_eq!(
        scene.interact_mechanism(0, &Vec3::new(36., 123., 0.)),
        Some(Some(506))
    );
    assert!(scene.mechanism_input_locked());
    scene.update_mechanisms(0.); // Already standing on the plate.
    assert_eq!(scene.take_mechanism_sounds(), vec!["We026"]);
    scene.update_mechanisms(0.); // Finish the zero-length test CVD.
    assert_eq!(scene.take_mechanism_sounds(), vec!["Wg005"]);
    scene.update_mechanisms(1.25);
    assert_eq!(
        scene
            .get_root_object(10)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y,
        119.5
    );
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let mut saved: PersistentState =
        serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    let restored = self::scene(nodes.clone());
    restored.restore_mechanisms(&saved);
    restored.update_mechanisms(1.25);
    assert_eq!(
        restored
            .get_root_object(10)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y,
        116.
    );
    // The plate releases its lock only after starting its linked CVD's lock.
    assert!(restored.mechanism_input_locked());
    assert_eq!(restored.take_mechanism_sounds(), vec!["Wg001"]);
    restored.update_mechanisms(0.);
    assert!(!restored.mechanism_input_locked());
    assert_eq!(
        restored.interact_mechanism(0, &Vec3::new(0., 123., 0.)),
        None
    );
    restored.snapshot_mechanisms(&mut saved);
    let again = self::scene(nodes);
    again.restore_mechanisms(&saved);
    again.update_mechanisms(100.);
    assert_eq!(
        again
            .get_root_object(10)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y,
        116.
    );
    assert!(!again.mechanism_input_locked());
    assert_eq!(
        saved.mechanism_states("m17", "7").unwrap()[&10].remaining,
        255
    );
}

#[test]
fn native_moving_target_uses_raw_yaw_easing_and_persists_full_position() {
    let mut mover = node(17, 34, 3);
    mover.position = Vec3::new(100., 123., 200.);
    mover.rotation = 82.;
    mover.ladder_nav_coord1.0 = 0;
    let mut gate = node(3, 22, u16::MAX);
    gate.sce_proc_id = 507;
    let nodes = vec![mover, gate];
    let scene = scene(nodes.clone());
    scene.trigger_mechanism(17);
    scene.update_mechanisms(2.);
    let halfway = scene
        .get_root_object(17)
        .unwrap()
        .transform()
        .borrow()
        .position();
    assert!((halfway.x - 156.980667).abs() < 0.0001);
    assert!((halfway.z - 218.793724).abs() < 0.0001);
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let restored = self::scene(nodes);
    restored.restore_mechanisms(&saved);
    restored.update_mechanisms(2.);
    let end = restored
        .get_root_object(17)
        .unwrap()
        .transform()
        .borrow()
        .position();
    assert_eq!(end.x, 213.961334);
    assert_eq!(end.z, 237.587448);
    assert_eq!(restored.update_mechanisms(1.), vec![507]);
    restored.trigger_mechanism(17);
    restored.update_mechanisms(4.);
    let second = restored
        .get_root_object(17)
        .unwrap()
        .transform()
        .borrow()
        .position();
    assert!((second.x - (end.x + 113.961334)).abs() < 0.0001);
}

#[test]
fn native_hazards_share_strict_cooldown_and_disabled_hammer_keeps_phase_and_height() {
    let mut hammer = node(5, 28, u16::MAX);
    hammer.position = Vec3::new(0., 10., 0.);
    hammer.ladder_nav_coord1 = (1, 145);
    let nodes = vec![hammer.clone(), ScnNode { index: 6, ..hammer }];
    let scene = scene(nodes.clone());
    cache_player(&scene, Vec3::new(0., 10., 0.));
    let actor = scene.get_role_entity(0).unwrap();
    attach_bounds(&actor, Vec3::new(-1., 0., -1.), Vec3::new(1., 1., 1.));
    for id in [5, 6] {
        attach_bounds(
            &scene.get_root_object(id).unwrap(),
            Vec3::new(-2., -200., -2.),
            Vec3::new(2., 200., 2.),
        );
    }
    let mut timer = 2.;
    assert_eq!(scene.update_hazard_damage(0., &mut timer), None);
    scene.trigger_mechanism(5);
    scene.trigger_mechanism(6);
    scene.update_mechanisms(0.5);
    timer = 1.2;
    assert_eq!(scene.update_hazard_damage(0., &mut timer), None);
    assert_eq!(scene.update_hazard_damage(0.001, &mut timer), Some(0.15));
    assert_eq!(scene.update_hazard_damage(0., &mut timer), None);
    let height = scene
        .get_root_object(5)
        .unwrap()
        .transform()
        .borrow()
        .position()
        .y;
    scene.trigger_mechanism(5);
    scene.update_mechanisms(0.5);
    assert_eq!(
        scene
            .get_root_object(5)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y,
        height
    );
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let restored = self::scene(nodes);
    restored.restore_mechanisms(&saved);
    assert_eq!(
        restored
            .get_root_object(5)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y,
        height
    );
    restored.trigger_mechanism(5);
    restored.update_mechanisms(0.5);
    let expected = 10. + (1.5f32.sin() + 1.) * 145. * 0.5;
    assert!(
        (restored
            .get_root_object(5)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y
            - expected)
            .abs()
            < 0.0001
    );
}

pub(super) struct UnusedFactory;

struct UnusedRenderObject(std::cell::RefCell<VertexBuffer>);

impl radiance::rendering::RenderObject for UnusedRenderObject {
    fn update_vertices(&self, update: &dyn Fn(std::cell::RefMut<VertexBuffer>)) {
        update(self.0.borrow_mut());
    }

    fn local_aabb(&self) -> Option<([f32; 3], [f32; 3])> {
        self.0.borrow().aabb_min_max()
    }
}

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
        vertices: VertexBuffer,
        _: Vec<u32>,
        _: &MaterialDef,
        _: bool,
    ) -> RenderObjectHandle {
        RenderObjectHandle::from_dyn(Rc::new(UnusedRenderObject(std::cell::RefCell::new(
            vertices,
        ))))
    }
    fn create_rendering_component(&self, objects: Vec<RenderObjectHandle>) -> RenderingComponent {
        let mut component = RenderingComponent::new();
        for object in objects {
            component.push_render_object(object);
        }
        component
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
        path_waypoints: vec![],
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
            scene_type: 0,
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

#[test]
fn linked_cvd_bridge_opens_navigation_only_after_animation_and_restores_once() {
    let scene = floor_scene(19);
    scene.update_mechanisms(0.);
    assert_eq!(scene.nav().get(0, 7, 12).unwrap().distance_to_border, 0);
    scene.trigger_mechanism(2);
    assert!(scene.mechanism_input_locked());
    scene.update_mechanisms(0.);
    assert!(!scene.mechanism_input_locked());
    assert_eq!(scene.nav().get(0, 7, 12).unwrap().distance_to_border, 2);
    assert_eq!(scene.nav().get(0, 9, 7).unwrap().distance_to_border, 0);
    assert_eq!(scene.nav().get(1, 7, 12).unwrap().distance_to_border, 0);
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let restored = floor_scene(19);
    restored.restore_mechanisms(&saved);
    assert_eq!(restored.nav().get(0, 7, 12).unwrap().distance_to_border, 2);
    restored.trigger_mechanism(2);
    assert!(!restored.mechanism_input_locked());
    restored.set_mechanism_active(2, false);
    assert_eq!(restored.nav().get(0, 7, 12).unwrap().distance_to_border, 0);
}

#[test]
fn plate_floor_has_inset_native_height_and_lowers_the_grounded_player() {
    let scene = floor_scene(29);
    let player = CoreEntity::create("test-player".into(), true);
    player
        .transform()
        .borrow_mut()
        .set_position(&Vec3::new(150., 34., 150.));
    scene.set_mechanism_player(Some((player.clone(), 0)));
    scene.set_mechanism_party_count(40);
    scene.update_mechanisms(0.);
    assert_eq!(scene.nav().get(0, 7, 7).unwrap().height, 34.);
    assert_eq!(scene.nav().get(0, 5, 7).unwrap().distance_to_border, 0);
    assert_eq!(scene.nav().get(0, 7, 2).unwrap().distance_to_border, 0);
    assert_eq!(
        scene.interact_mechanism(0, &player.transform().borrow().position()),
        Some(None)
    );
    scene.update_mechanisms(0.);
    scene.update_mechanisms(0.);
    scene.update_mechanisms(1.25);
    assert_eq!(scene.nav().get(0, 7, 7).unwrap().height, 30.5);
    assert_eq!(player.transform().borrow().position().y, 30.5);
    scene.update_mechanisms(1.25);
    assert_eq!(scene.nav().get(0, 7, 7).unwrap().height, 27.);
    assert_eq!(player.transform().borrow().position().y, 27.);
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let restored = floor_scene(29);
    restored.restore_mechanisms(&saved);
    assert_eq!(restored.nav().get(0, 7, 7).unwrap().height, 27.);
}

#[test]
fn platform_and_mover_floors_never_lower_authored_nav_ground() {
    for kind in [22, 34] {
        for ground in [0., 42.] {
            let scene = floor_scene_with_height(kind, ground);
            scene.update_mechanisms(0.);
            assert_eq!(scene.nav().get(0, 7, 7).unwrap().height, ground.max(10.));
        }
    }
}

#[test]
fn plate_approach_cancels_when_controlled_player_is_replaced() {
    let mut plate = node(10, 29, u16::MAX);
    plate.ladder_nav_coord1.0 = 1;
    let scene = scene(vec![plate]);
    cache_player(&scene, Vec3::new(0., 10., 0.));
    assert_eq!(
        scene.interact_mechanism(0, &Vec3::new(0., 10., 0.)),
        Some(None)
    );
    assert!(scene.mechanism_input_locked());
    scene.set_mechanism_player(None);
    scene.update_mechanisms(1.);
    assert!(!scene.mechanism_input_locked());
    assert!(scene.take_mechanism_sounds().is_empty());
}

fn floor_scene(kind: u16) -> ScnScene {
    floor_scene_with_height(kind, 0.)
}

fn floor_scene_with_height(kind: u16, height: f32) -> ScnScene {
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
                            height,
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
        Vec3::new(if kind == 19 { -80. } else { -40. }, 0., -100.),
        Vec3::new(if kind == 19 { 80. } else { 40. }, 1., 100.),
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

fn path_node(mode: i32) -> ScnNode {
    let mut path = node(5, 5, u16::MAX);
    path.switch_state = 1;
    path.ladder_nav_coord1.0 = mode;
    path.path_waypoints = vec![
        Vec3::new(0., 10., 0.),
        Vec3::new(100., 10., 0.),
        Vec3::new(100., 10., 100.),
    ];
    path.d84 = path.path_waypoints.len() as u32;
    path
}

fn transport_scene(path: ScnNode, passable: bool) -> ScnScene {
    let point = NavMapPoint {
        height: 10.,
        distance_to_border: if passable { 2 } else { 0 },
    };
    scene_with_nav(
        vec![path],
        "m17",
        NavFile {
            version: 1,
            maps: vec![NavMap {
                layer_triggers: None,
                min_coord: Vec3::new(-200., 0., -200.),
                max_coord: Vec3::new(400., 0., 400.),
                width: 29,
                height: 29,
                map: vec![vec![point; 29]; 29],
            }],
            unknown2: vec![],
        },
        crate::GameType::PAL3,
    )
}

fn cache_player(scene: &ScnScene, position: Vec3) {
    let player = scene.get_role_entity(0).unwrap();
    player.transform().borrow_mut().set_position(&position);
    scene.set_mechanism_player(Some((player, 0)));
}

#[test]
#[ignore = "requires original game assets via PAL3_ASSET_PATH"]
fn shipped_pickup_container_animates_before_reward_and_restores_without_replaying_sound() {
    use crate::openpal3::comdef::IScnSceneComponent;
    let path = std::env::var("PAL3_ASSET_PATH").expect("PAL3_ASSET_PATH");
    let assets = Rc::new(AssetManager::new(
        Rc::new(UnusedFactory),
        Rc::new(packfs::init_virtual_fs(std::path::Path::new(&path), None)),
    ));
    for sound in ["wa006", "wg011"] {
        assert!(!assets.load_snd_data(sound).unwrap().is_empty());
    }
    let root = assets.load_scn("m01", "1");
    let component = root.get_component(IScnSceneComponent::uuid()).unwrap();
    component.on_loading();
    let component = component.query_interface::<IScnSceneComponent>().unwrap();
    let scene = component.inner::<ScnScene>();
    let entity = scene.get_root_object(83).unwrap();
    let duration = super::cvd_entity::animation_duration(&entity);
    assert!(duration > 0.5, "mesh frames outlast the transform tracks");
    let position = entity.transform().borrow().position();
    cache_player(scene, position);
    assert_eq!(scene.interact_mechanism(0, &position), Some(None));
    scene.update_mechanisms(0.);
    assert!(scene.mechanism_input_locked());
    assert!(entity.visible());
    assert!(scene.take_pickup_rewards().is_empty());
    assert_eq!(scene.take_mechanism_sounds(), vec!["wg011"]);
    scene.update_mechanisms(duration * 0.25);
    assert!(scene.take_pickup_rewards().is_empty());

    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    assert_eq!(
        saved.mechanism_states("m01", "1").unwrap()[&83].animation_time,
        Some(duration * 0.5)
    );
    let saved = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    scene.restore_mechanisms(&saved);
    scene.update_mechanisms(duration * 0.25);
    assert_eq!(scene.take_pickup_rewards(), vec![(8101, 0)]);
    assert!(!scene.mechanism_input_locked());
    assert!(!entity.visible());
    assert!(scene.take_mechanism_sounds().is_empty());
    let mut claimed = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut claimed);
    scene.restore_mechanisms(&claimed);
    scene.update_mechanisms(1.);
    assert!(scene.take_pickup_rewards().is_empty());
    assert!(scene.take_mechanism_sounds().is_empty());
}

#[test]
fn ground_pickup_queues_reward_once_hides_and_persists_claim() {
    let mut pickup = node(41, 41, u16::MAX);
    pickup.ladder_nav_coord1.0 = 6601;
    pickup.ladder_nav_coord2.0 = 25;
    let scene = scene(vec![pickup.clone()]);
    cache_player(&scene, Vec3::new(0., 10., 0.));
    assert_eq!(
        scene.interact_mechanism(0, &Vec3::new(0., 10., 0.)),
        Some(None)
    );
    scene.update_mechanisms(0.);
    assert_eq!(scene.take_pickup_rewards(), vec![(6601, 25)]);
    assert!(scene.take_pickup_rewards().is_empty());
    assert!(!scene.get_root_object(41).unwrap().visible());

    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let restored = self::scene(vec![pickup]);
    restored.restore_mechanisms(&saved);
    cache_player(&restored, Vec3::new(0., 10., 0.));
    assert!(!restored.get_root_object(41).unwrap().visible());
    assert_eq!(
        restored.interact_mechanism(0, &Vec3::new(0., 10., 0.)),
        None
    );
    restored.update_mechanisms(1.);
    assert!(restored.take_pickup_rewards().is_empty());
}

#[test]
fn ordinary_pickup_uses_native_money_fallback_and_persists_removal() {
    for (mode, money, expected) in [(0, 200, 200), (1, 200, 200), (23, 200, 23), (-25, 200, -25)] {
        let mut pickup = node(24, 24, u16::MAX);
        pickup.ladder_nav_coord1 = (6201, mode);
        pickup.ladder_nav_coord2.0 = money;
        let scene = scene(vec![pickup.clone()]);
        cache_player(&scene, Vec3::new(0., 10., 0.));
        assert_eq!(
            scene.interact_mechanism(0, &Vec3::new(0., 10., 0.)),
            Some(None)
        );
        scene.update_mechanisms(0.);
        assert_eq!(scene.take_pickup_rewards(), vec![(6201, expected)]);
        assert!(!scene.get_root_object(24).unwrap().visible());
        let mut saved = PersistentState::new("OpenPAL3".into());
        scene.snapshot_mechanisms(&mut saved);
        let restored = self::scene(vec![pickup]);
        restored.restore_mechanisms(&saved);
        cache_player(&restored, Vec3::new(0., 10., 0.));
        assert_eq!(
            restored.interact_mechanism(0, &Vec3::new(0., 10., 0.)),
            None
        );
        restored.update_mechanisms(1.);
        assert!(restored.take_pickup_rewards().is_empty());
    }
}

#[test]
fn ordinary_pickup_waits_for_its_script_before_granting() {
    let mut pickup = node(24, 24, u16::MAX);
    pickup.ladder_nav_coord1 = (6201, 0);
    pickup.sce_proc_id = 500;
    let scene = scene(vec![pickup]);
    cache_player(&scene, Vec3::new(0., 10., 0.));
    assert_eq!(
        scene.interact_mechanism(0, &Vec3::new(0., 10., 0.)),
        Some(Some(500))
    );
    scene.set_mechanism_script_running(true);
    scene.update_mechanisms(1.);
    assert!(scene.mechanism_input_locked());
    assert!(scene.take_pickup_rewards().is_empty());
    scene.set_mechanism_script_running(false);
    scene.update_mechanisms(0.);
    assert!(!scene.mechanism_input_locked());
    assert_eq!(scene.take_pickup_rewards(), vec![(6201, 0)]);
}

#[test]
fn pickup_container_starts_its_script_without_waiting_for_it_to_finish() {
    let mut pickup = node(41, 41, u16::MAX);
    pickup.ladder_nav_coord1.0 = 6201;
    pickup.sce_proc_id = 500;
    let scene = scene(vec![pickup]);
    cache_player(&scene, Vec3::new(0., 10., 0.));
    assert_eq!(
        scene.interact_mechanism(0, &Vec3::new(0., 10., 0.)),
        Some(Some(500))
    );
    scene.set_mechanism_script_running(true);
    scene.update_mechanisms(0.);
    assert_eq!(scene.take_mechanism_sounds(), vec!["wg011"]);
    assert_eq!(scene.take_pickup_rewards(), vec![(6201, 0)]);
    assert!(!scene.mechanism_input_locked());
}

#[test]
fn colored_chest_uses_inclusive_native_role_color_thresholds() {
    let thresholds = [
        [50, 50, 50, 80, 30],
        [50, 50, 30, 50, 80],
        [80, 30, 50, 50, 50],
        [30, 50, 80, 50, 50],
        [50, 80, 50, 30, 50],
    ];
    for (color, thresholds) in thresholds.iter().enumerate() {
        for (role, &threshold) in thresholds.iter().enumerate() {
            let mut chest = ChestState {
                attempts: 0,
                phase: ChestPhase::Closed,
            };
            assert!(chest.try_open(color, role, threshold + 100));
            assert_eq!(chest.attempts, 0);
            assert!(!chest.try_open(color, role, threshold + 1));
            assert_eq!(chest.attempts, 1);
        }
    }
}

fn restore_chest_phase(scene: &ScnScene, node: &ScnNode, attempts: u8, phase: ChestPhase) {
    let mut mechanism = Mechanism::from_node(node).unwrap();
    mechanism.state.chest = Some(ChestState { attempts, phase });
    let mut saved = PersistentState::new("OpenPAL3".into());
    saved.set_mechanism_states(
        "m17",
        "7",
        std::collections::HashMap::from([(node.index, mechanism.saved_state())]),
    );
    scene.restore_mechanisms(&saved);
}

#[test]
fn colored_chest_settles_all_six_items_and_bonus_or_penalty_once() {
    for role in [0, 1] {
        for attempts in 0..=3 {
            let mut chest = node(26, 26, u16::MAX);
            chest.ladder_nav_coord1 = (82111, 82202);
            chest.ladder_nav_coord2 = (84102, 83102);
            chest.ladder_switch_layer = 6201;
            chest.b49 = 6112_i32.to_le_bytes().to_vec();
            let scene = scene(vec![chest.clone()]);
            restore_chest_phase(
                &scene,
                &chest,
                attempts,
                ChestPhase::Opening { role, elapsed: 0. },
            );
            assert!(scene.mechanism_input_locked());
            scene.update_mechanisms(0.);
            assert!(!scene.mechanism_input_locked());
            let items = scene.take_pickup_rewards();
            if attempts < 3 {
                assert_eq!(
                    items,
                    vec![
                        (82111, 0),
                        (82202, 0),
                        (84102, 0),
                        (83102, 0),
                        (6201, 0),
                        (6112, 0)
                    ]
                );
            } else {
                assert!(items.is_empty());
            }
            assert_eq!(
                scene.take_chest_rewards(),
                vec![ChestReward {
                    role,
                    experience: [55, 33, 11, 0][attempts as usize],
                    favor: if attempts == 3 && role != 0 { -2 } else { 0 },
                }]
            );
            let mut saved = PersistentState::new("OpenPAL3".into());
            scene.snapshot_mechanisms(&mut saved);
            let saved: PersistentState =
                serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
            let restored = self::scene(vec![chest]);
            restored.restore_mechanisms(&saved);
            cache_player(&restored, Vec3::new(0., 10., 0.));
            assert_eq!(
                restored.interact_mechanism(0, &Vec3::new(0., 10., 0.)),
                None
            );
            restored.update_mechanisms(1.);
            assert!(restored.take_pickup_rewards().is_empty());
            assert!(restored.take_chest_rewards().is_empty());
        }
    }
}

#[test]
fn colored_chest_packed_color_does_not_change_navigation_layer() {
    let mut chest = node(26, 26, u16::MAX);
    chest.name = "wz02_3.cvd".into();
    chest.nav_layer = 0x401;
    let mechanism = Mechanism::from_node(&chest).unwrap();
    assert_eq!(mechanism.layer, 1);
    let scene = scene(vec![chest]);
    cache_player(&scene, Vec3::new(0., 10., 0.));
    assert_eq!(scene.interact_mechanism(0, &Vec3::new(0., 10., 0.)), None);
    assert_eq!(
        scene.interact_mechanism(1, &Vec3::new(0., 10., 0.)),
        Some(None)
    );
    assert!(scene.mechanism_input_locked());
    scene.set_mechanism_player(None);
    scene.update_mechanisms(1.);
    assert!(!scene.mechanism_input_locked());
    assert!(scene.take_pickup_rewards().is_empty());
    assert!(scene.take_chest_rewards().is_empty());
}

#[test]
fn ordinary_pickup_and_chest_do_not_interact_when_hidden_or_exhausted() {
    for kind in [24, 26] {
        let mut object = node(kind, kind, u16::MAX);
        object.remaining_triggers = 0;
        let scene = scene(vec![object]);
        cache_player(&scene, Vec3::new(0., 10., 0.));
        assert_eq!(scene.interact_mechanism(0, &Vec3::new(0., 10., 0.)), None);
        let scene = self::scene(vec![node(kind, kind, u16::MAX)]);
        cache_player(&scene, Vec3::new(0., 10., 0.));
        scene
            .get_root_object(kind as i32)
            .unwrap()
            .set_visible(false);
        assert_eq!(scene.interact_mechanism(0, &Vec3::new(0., 10., 0.)), None);
        scene.update_mechanisms(1.);
        assert!(scene.take_pickup_rewards().is_empty());
    }
}

#[test]
fn pickup_cancels_cleanly_when_player_disappears() {
    let scene = scene(vec![node(41, 41, u16::MAX)]);
    cache_player(&scene, Vec3::new(150., 10., 0.));
    assert_eq!(
        scene.interact_mechanism(0, &Vec3::new(150., 10., 0.)),
        Some(None)
    );
    scene.set_mechanism_player(None);
    scene.update_mechanisms(0.);
    assert!(scene.take_pickup_rewards().is_empty());
    assert!(!scene.mechanism_input_locked());
}

#[test]
fn scripted_item_trigger_uses_live_visible_position_proc_and_layer() {
    let mut item = node(12, 11, u16::MAX);
    item.sce_proc_id = 500;
    let item_scene = scene(vec![item]);
    item_scene.load_objects_for_test();
    cache_player(&item_scene, Vec3::new(0., 10., 0.));
    let entity = item_scene.get_root_object(12).unwrap();
    entity
        .transform()
        .borrow_mut()
        .set_position(&Vec3::new(75., 10., 0.));
    assert_eq!(
        item_scene.test_item_trigger(&Vec3::new(75., 10., 0.)),
        Some(500)
    );
    entity.set_visible(false);
    assert_eq!(item_scene.test_item_trigger(&Vec3::new(75., 10., 0.)), None);
    entity.set_visible(true);
    item_scene.set_mechanism_player(Some((item_scene.get_role_entity(0).unwrap(), 1)));
    assert_eq!(item_scene.test_item_trigger(&Vec3::new(75., 10., 0.)), None);

    let mut zero = node(13, 16, u16::MAX);
    zero.sce_proc_id = 0;
    let zero_scene = scene(vec![zero]);
    zero_scene.load_objects_for_test();
    cache_player(&zero_scene, Vec3::new(0., 10., 0.));
    assert_eq!(zero_scene.test_item_trigger(&Vec3::new(0., 10., 0.)), None);
}

#[test]
fn scripted_teapot_becomes_interactable_when_the_plot_activates_it() {
    let mut teapot = node(6, 16, u16::MAX);
    teapot.active = 0;
    teapot.sce_proc_id = 1901;
    teapot.position = Vec3::new(24.837112, 44.123825, -117.490524);
    let position = teapot.position;
    let item_scene = scene(vec![teapot]);
    item_scene.load_objects_for_test();
    cache_player(&item_scene, position);
    let entity = item_scene.get_root_object(6).unwrap();

    entity.set_visible(false);
    assert_eq!(item_scene.test_item_trigger(&position), None);
    entity.set_visible(true);
    assert_eq!(item_scene.test_item_trigger(&position), Some(1901));
    entity.set_visible(false);
    assert_eq!(item_scene.test_item_trigger(&position), None);
}

#[test]
fn scripted_teapot_visibility_survives_save_and_scene_reload() {
    let mut teapot = node(6, 16, u16::MAX);
    teapot.active = 0;
    teapot.sce_proc_id = 1901;
    let position = teapot.position;
    let item_scene = scene(vec![teapot.clone()]);
    item_scene.load_objects_for_test();
    let mut saved = PersistentState::new("OpenPAL3".into());

    for visible in [true, false] {
        item_scene.get_root_object(6).unwrap().set_visible(visible);
        item_scene.snapshot_mechanisms(&mut saved);
        let saved: PersistentState =
            serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        let restored = scene(vec![teapot.clone()]);
        restored.load_objects_for_test();
        restored.get_root_object(6).unwrap().set_visible(!visible);
        restored.restore_mechanisms(&saved);
        cache_player(&restored, position);
        assert_eq!(restored.get_root_object(6).unwrap().visible(), visible);
        assert_eq!(
            restored.test_item_trigger(&position),
            visible.then_some(1901)
        );
    }
}

#[test]
fn ground_pickup_aborts_without_reward_when_navigation_approach_is_blocked() {
    let mut pickup = node(41, 41, u16::MAX);
    pickup.position = Vec3::new(100., 10., 0.);
    pickup.ladder_nav_coord1.0 = 6601;
    let blocked = NavMapPoint {
        height: 10.,
        distance_to_border: 0,
    };
    let scene = scene_with_nav(
        vec![pickup],
        "m17",
        NavFile {
            version: 1,
            maps: vec![NavMap {
                layer_triggers: None,
                min_coord: Vec3::new(-200., 0., -200.),
                max_coord: Vec3::new(400., 0., 400.),
                width: 29,
                height: 29,
                map: vec![vec![blocked; 29]; 29],
            }],
            unknown2: vec![],
        },
        crate::GameType::PAL3,
    );
    cache_player(&scene, Vec3::new(0., 10., 0.));
    assert_eq!(
        scene.interact_mechanism(0, &Vec3::new(0., 10., 0.)),
        Some(None)
    );
    scene.update_mechanisms(2.);
    assert!(scene.take_pickup_rewards().is_empty());
    assert!(!scene.mechanism_input_locked());
    assert_eq!(
        scene
            .get_role_entity(0)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .x,
        0.
    );
    assert!(scene.get_root_object(41).unwrap().visible());
}

#[test]
fn ordinary_path_uses_native_linear_knot_tangent_carries_and_disembarks() {
    let scene = transport_scene(path_node(0), true);
    cache_player(&scene, Vec3::new(0., 10., 0.));
    assert_eq!(
        scene.interact_mechanism(0, &Vec3::new(0., 10., 0.)),
        Some(None)
    );
    scene.update_mechanisms(0.);
    scene.update_mechanisms(1.);
    let platform = scene.get_root_object(5).unwrap();
    let position = platform.transform().borrow().position();
    assert_eq!((position.x, position.z), (100., 0.));
    let player = scene.get_role_entity(0).unwrap();
    {
        let transform = player.transform();
        let transform = transform.borrow();
        let matrix = transform.matrix();
        assert!(matrix[0][2].abs() > matrix[2][2].abs());
    }
    assert!(scene.mechanism_input_locked());
    scene.update_mechanisms(1.01);
    scene.update_mechanisms(1.);
    assert!(!scene.mechanism_input_locked());
    assert_eq!(
        {
            let p = platform.transform().borrow().position();
            (p.x, p.z)
        },
        (100., 100.)
    );
}

#[test]
fn ordinary_path_blocked_at_both_ends_reverses_again_at_start() {
    let scene = transport_scene(path_node(0), false);
    cache_player(&scene, Vec3::new(0., 10., 0.));
    scene.interact_mechanism(0, &Vec3::new(0., 10., 0.));
    scene.update_mechanisms(0.);
    scene.update_mechanisms(2.01);
    assert!(scene.mechanism_input_locked());
    scene.update_mechanisms(2.01);
    assert!(scene.mechanism_input_locked());
    let position = scene
        .get_root_object(5)
        .unwrap()
        .transform()
        .borrow()
        .position();
    assert_eq!((position.x, position.z), (0., 0.));
    scene.update_mechanisms(0.5);
    let position = scene
        .get_root_object(5)
        .unwrap()
        .transform()
        .borrow()
        .position();
    assert_eq!((position.x, position.z), (50., 0.));
}

#[test]
fn path_transport_releases_input_if_controlled_player_disappears_mid_motion() {
    let scene = transport_scene(path_node(0), true);
    cache_player(&scene, Vec3::new(0., 10., 0.));
    scene.interact_mechanism(0, &Vec3::new(0., 10., 0.));
    scene.update_mechanisms(0.);
    scene.update_mechanisms(0.5);
    assert!(scene.mechanism_input_locked());
    scene.set_mechanism_player(None);
    scene.update_mechanisms(0.);
    assert!(!scene.mechanism_input_locked());
}

#[test]
fn ordinary_path_restores_inflight_without_reboarding_or_spending_again() {
    let mut path = path_node(0);
    path.remaining_triggers = 2;
    let scene = transport_scene(path.clone(), true);
    cache_player(&scene, Vec3::new(0., 10., 0.));
    scene.interact_mechanism(0, &Vec3::new(0., 10., 0.));
    scene.update_mechanisms(0.);
    scene.update_mechanisms(0.5);
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let restored = transport_scene(path, true);
    cache_player(&restored, Vec3::new(50., 10., 0.));
    restored.restore_mechanisms(&saved);
    assert!(restored.mechanism_input_locked());
    restored.update_mechanisms(1.51);
    restored.update_mechanisms(1.);
    assert!(!restored.mechanism_input_locked());
    let mut final_state = PersistentState::new("OpenPAL3".into());
    restored.snapshot_mechanisms(&mut final_state);
    assert_eq!(
        final_state.mechanism_states("m17", "7").unwrap()[&5].remaining,
        1
    );
}

#[test]
fn switch_link_moves_empty_path_transport_without_generic_trigger() {
    let mut switch = node(9, 23, 5);
    switch.ladder_nav_coord1 = (1, 1);
    let scene = transport_scene_with_nodes(vec![switch, {
        let mut path = path_node(0);
        path.switch_state = 0;
        path
    }]);
    scene.trigger_mechanism(9);
    scene.update_mechanisms(0.);
    scene.update_mechanisms(0.5);
    let position = scene
        .get_root_object(5)
        .unwrap()
        .transform()
        .borrow()
        .position();
    assert_eq!((position.x, position.z), (50., 0.));
    assert!(!scene.mechanism_input_locked());
}

fn transport_scene_with_nodes(nodes: Vec<ScnNode>) -> ScnScene {
    let point = NavMapPoint {
        height: 10.,
        distance_to_border: 2,
    };
    scene_with_nav(
        nodes,
        "m17",
        NavFile {
            version: 1,
            maps: vec![NavMap {
                layer_triggers: None,
                min_coord: Vec3::new(-200., 0., -200.),
                max_coord: Vec3::new(400., 0., 400.),
                width: 29,
                height: 29,
                map: vec![vec![point; 29]; 29],
            }],
            unknown2: vec![],
        },
        crate::GameType::PAL3,
    )
}

#[test]
fn free_path_mode_selects_segment_and_uses_ninety_units_per_second() {
    let scene = transport_scene(path_node(1), true);
    cache_player(&scene, Vec3::new(0., 10., 0.));
    scene.interact_mechanism(0, &Vec3::new(0., 10., 0.));
    scene.update_mechanisms(0.);
    scene.update_mechanisms(0.);
    scene.update_mechanisms(0.5);
    let position = scene
        .get_root_object(5)
        .unwrap()
        .transform()
        .borrow()
        .position();
    assert!((position.x - 45.).abs() < 0.001);
    assert_eq!(position.z, 0.);
    scene.update_mechanisms(1.);
    scene.update_mechanisms(1.);
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let transport = saved.mechanism_states("m17", "7").unwrap()[&5]
        .platform
        .as_ref()
        .unwrap()
        .transport
        .as_ref()
        .unwrap();
    assert!(
        !scene.mechanism_input_locked(),
        "phase={:?} rider={:?} exit={:?}",
        transport.phase,
        transport.rider,
        transport.exit
    );
}
