use std::cell::RefCell;

use crosscom::ComRc;
use fileformats::pal4::gob::{
    GobEntry, GobFixedProperty, GobGetItemProperty, GobGomTaskProperty, GobMachineProperty,
    GobObjectType, GobProperty,
};
use radiance::{
    comdef::{IArmatureComponentExt, IEntity, IEntityExt},
    components::mesh::skinned_mesh::AnimationState,
};
use serde::{Deserialize, Serialize};

use super::{
    gameplay::GameplayCatalog,
    object_component::Pal4ObjectComponent,
    scene::{TAG_OBJECT, object_armature, object_component, play_object_animation},
    states::persistent_state::Pal4PersistentState,
    vm_context::Pal4VmContext,
};
use radiance::comdef::ISceneExt;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Reward {
    Property(i32, i32, bool),
    Equipment(i32, Option<i32>, bool),
    Money(i32, bool),
}

impl Reward {
    pub(crate) fn apply(
        self,
        state: &mut Pal4PersistentState,
        catalog: &GameplayCatalog,
        show: bool,
    ) -> anyhow::Result<(Option<String>, Option<&'static str>)> {
        let (template, name, count, sound) = match self {
            Self::Property(id, amount, remove) => {
                let definition = catalog
                    .properties
                    .get(&id)
                    .ok_or_else(|| anyhow::anyhow!("unknown PAL4 property {id}"))?;
                state.add_property(
                    id,
                    if remove {
                        amount.saturating_neg()
                    } else {
                        amount
                    },
                    catalog,
                )?;
                (
                    if remove { 252 } else { 251 },
                    Some(definition.name.as_str()),
                    Some(amount),
                    show.then_some(if remove { "WA07.wav" } else { "WA06.wav" }),
                )
            }
            Self::Equipment(id, count, remove) => {
                let definition = catalog
                    .equipment
                    .get(&id)
                    .ok_or_else(|| anyhow::anyhow!("unknown PAL4 equipment {id}"))?;
                if remove {
                    if state.equipment_count(id) == 0 {
                        return Ok((None, None));
                    }
                    state.remove_equipment(id, 1);
                } else {
                    state.add_equipment(id, 1);
                }
                (
                    if remove { 126 } else { 251 },
                    Some(definition.name.as_str()),
                    if remove {
                        None
                    } else {
                        Some(count.unwrap_or(1))
                    },
                    show.then_some(if remove { "WA07.wav" } else { "WA06.wav" }),
                )
            }
            Self::Money(amount, remove) => {
                state.add_money(if remove {
                    amount.saturating_neg()
                } else {
                    amount
                });
                (
                    if remove { 267 } else { 194 },
                    None,
                    Some(amount),
                    (!remove).then_some("WA40.wav"),
                )
            }
        };
        let text = show
            .then(|| {
                let mut text = catalog
                    .game_strings
                    .get(&template)
                    .cloned()
                    .unwrap_or_default();
                if let Some(name) = name {
                    text = text.replacen("%s", name, 1);
                }
                if let Some(count) = count {
                    text = text.replacen("%d", &count.to_string(), 1);
                }
                text
            })
            .filter(|text| !text.is_empty());
        Ok((text, sound))
    }
}

#[derive(Debug, Clone)]
pub(crate) enum RewardTask {
    Animation(String),
    Grant(Reward),
}

#[derive(Debug, Clone)]
pub(crate) struct RewardPlan {
    pub tasks: Vec<RewardTask>,
    pub chest: bool,
    pub stored: bool,
    pub disappear: bool,
    pub valid_for: i32,
    pub opening_sound: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RewardProgress {
    pub remaining: i32,
    pub next_task: Option<usize>,
}

pub(crate) struct ObjectRewards {
    pub plan: RewardPlan,
    pub progress: RewardProgress,
    pub waiting_animation: bool,
}

impl ObjectRewards {
    pub fn from_entry(entry: &GobEntry, kind: u32) -> Option<RefCell<Self>> {
        if !entry.research_function.to_string().ok()?.is_empty() {
            return None;
        }
        let integer = |name, default| {
            entry
                .get_property(name)
                .or_else(|| entry.get_parameter(name))
                .and_then(GobProperty::value_i32)
                .unwrap_or(default)
        };
        let mut tasks = Vec::new();
        match kind {
            GobObjectType::GET_ITEM => {
                let property = integer(GobGetItemProperty::PROP_ID, 0);
                let equipment = integer(GobGetItemProperty::EQUIP_ID, 0);
                let count = integer(GobGetItemProperty::NUM, 1);
                if property > 0 {
                    tasks.push(RewardTask::Grant(Reward::Property(property, count, false)));
                } else if equipment > 0 {
                    tasks.push(RewardTask::Grant(Reward::Equipment(
                        equipment,
                        Some(count),
                        false,
                    )));
                }
            }
            GobObjectType::MACHINE => {
                if integer(GobMachineProperty::REPEAT, 0) != 0
                    || integer(GobMachineProperty::AUTO_TOUCH, 0) != 0
                {
                    return None;
                }
                for property in &entry.parameters {
                    if let GobProperty::GobPropertyObjectArray(array) = property {
                        if property.name() == GobMachineProperty::CONDITION {
                            if array.0.iter().any(|condition| {
                                condition
                                    .get_property(GobMachineProperty::PROPID)
                                    .and_then(GobProperty::value_i32)
                                    .is_some_and(|id| id > 0)
                            }) {
                                return None;
                            }
                        } else if property
                            .name()
                            .starts_with(GobMachineProperty::TASK_LIST_PREFIX)
                        {
                            for task in &array.0 {
                                let int = |name, default| {
                                    task.get_property(name)
                                        .and_then(GobProperty::value_i32)
                                        .unwrap_or(default)
                                };
                                if int(GobGomTaskProperty::WAIT_MODE, 1) != 1
                                    || [
                                        GobGomTaskProperty::WAIT_TIME,
                                        GobGomTaskProperty::EXPT_TIME,
                                    ]
                                    .iter()
                                    .any(|name| {
                                        task.get_property(name)
                                            .and_then(GobProperty::value_f32)
                                            .unwrap_or(0.)
                                            != 0.
                                    })
                                {
                                    return None;
                                }
                                tasks.push(match int(GobGomTaskProperty::TYPE, -1) {
                                    3 if int(GobGomTaskProperty::ACTION_TIME, 1) == 1 => {
                                        RewardTask::Animation(
                                            task.get_property(GobGomTaskProperty::ACTION_NAME)?
                                                .value_string()?
                                                .to_owned(),
                                        )
                                    }
                                    7 => RewardTask::Grant(Reward::Property(
                                        int(GobGomTaskProperty::GETITEM_ID, 0),
                                        int(GobGomTaskProperty::GETITEM_NUM, 1),
                                        false,
                                    )),
                                    8 => RewardTask::Grant(Reward::Equipment(
                                        int(GobGomTaskProperty::GETITEM_ID, 0),
                                        Some(1),
                                        false,
                                    )),
                                    9 => RewardTask::Grant(Reward::Money(
                                        int("PAL4_GOMTask_getitem_money", 0),
                                        false,
                                    )),
                                    _ => return None,
                                });
                            }
                        }
                    }
                }
                if !tasks
                    .iter()
                    .any(|task| matches!(task, RewardTask::Grant(_)))
                {
                    return None;
                }
            }
            _ => return None,
        }
        Some(RefCell::new(Self {
            plan: RewardPlan {
                tasks,
                chest: kind == GobObjectType::MACHINE,
                stored: integer(GobFixedProperty::STORE, 0) != 0,
                disappear: integer(GobFixedProperty::AUTO_DISAPPEAR, 0) != 0,
                valid_for: integer(GobFixedProperty::VALIDFOR, -1),
                opening_sound: integer(GobFixedProperty::MARKER, 0) == 4,
            },
            progress: RewardProgress {
                remaining: integer(GobFixedProperty::RESEARCH_NUM, -1),
                next_task: None,
            },
            waiting_animation: false,
        }))
    }

    pub fn available(&self) -> bool {
        self.progress.remaining != 0 && self.progress.next_task.is_none()
    }

    fn start(&mut self, leader: usize) -> Result<bool, i32> {
        if !self.available() {
            return Ok(false);
        }
        // Native 0x425950 restricts only Lingsha (1) and Ziying (3).
        if self.plan.valid_for != leader as i32 {
            match self.plan.valid_for {
                1 => return Err(265),
                3 => return Err(266),
                _ => {}
            }
        }
        if self.progress.remaining > 0 {
            self.progress.remaining -= 1;
        }
        self.progress.next_task = Some(0);
        Ok(true)
    }
}

impl Pal4VmContext {
    pub(crate) fn change_reward(&mut self, reward: Reward, show: bool) {
        let result = {
            let handle = self.session_handle();
            let mut session = handle.borrow_mut();
            let catalog = session.catalog();
            reward.apply(session.state_mut(), &catalog, show)
        };
        match result {
            Ok((text, sound)) => {
                if let Some(text) = text {
                    let _ = self.loader.dialog_artwork();
                    self.session_handle().borrow_mut().show_reward_notice(text);
                }
                if let Some(sound) = sound {
                    if let Err(error) = self.play_sound(sound.trim_end_matches(".wav")) {
                        log::warn!("PAL4 reward sound: {error:#}");
                    }
                }
            }
            Err(error) => log::warn!("PAL4 reward: {error:#}"),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::super::{
        gameplay::{CharacterStats, EquipmentDefinition, EquipmentType, PropertyDefinition},
        session::Pal4Session,
        states::persistent_state::PAL4_APP_NAME,
    };
    use super::*;
    use common::store_ext::StoreExt2;
    use fileformats::pal4::gob::GobPropertyI32;

    fn catalog() -> GameplayCatalog {
        let mut catalog = GameplayCatalog::default();
        catalog.properties.insert(
            3000,
            PropertyDefinition {
                id: 3000,
                name: "medicine".into(),
                description: String::new(),
                detail: String::new(),
                category: 0,
                target: 0,
                model: String::new(),
                texture: String::new(),
                usable_by: [true; 4],
                usable_in_menu: true,
                droppable: true,
                price: 0,
                skill_id: 0,
            },
        );
        catalog.equipment.insert(
            3500,
            EquipmentDefinition {
                id: 3500,
                name: "sword".into(),
                description: String::new(),
                kind: EquipmentType::Sword,
                model: String::new(),
                texture: String::new(),
                level_limits: [0; 4],
                price: 0,
                skill_id: 0,
                stats: CharacterStats::default(),
            },
        );
        catalog.game_strings.extend([
            (251, "gain %s x%d".into()),
            (252, "lose %s x%d".into()),
            (126, "lose %s".into()),
            (194, "gain money %d".into()),
            (267, "pay money %d".into()),
        ]);
        catalog
    }

    fn pickup() -> GobEntry {
        let integer = |name: &str, value| {
            GobProperty::GobPropertyI32(GobPropertyI32 {
                ty: 1,
                name: name.into(),
                value,
            })
        };
        GobEntry {
            name: "item001".into(),
            folder: "".into(),
            file_name: "DFB".into(),
            file_name2: "".into(),
            position: [0.; 3],
            rotation: [0.; 3],
            research_function: "".into(),
            flags: [1, 1, 1],
            trigger_distance: 60.,
            active: 1,
            game_object: GobPropertyI32 {
                ty: 1,
                name: "".into(),
                value: 3,
            },
            properties: vec![
                integer(GobFixedProperty::STORE, 1),
                integer(GobFixedProperty::RESEARCH_NUM, 1),
                integer(GobFixedProperty::AUTO_DISAPPEAR, 1),
            ],
            parameters_begin: GobPropertyI32 {
                ty: 1,
                name: "".into(),
                value: 1,
            },
            parameters: vec![
                integer(GobGetItemProperty::PROP_ID, 3000),
                integer(GobGetItemProperty::EQUIP_ID, 3500),
                integer(GobGetItemProperty::NUM, 5),
            ],
        }
    }

    #[test]
    fn actor_height_tolerance_merges_local_spheres_and_cancels_root_transform() {
        use super::super::asset_loader::AssetLoader;
        use fileformats::rwbs::{
            ChunkHeader, ChunkType, FormatFlag, Vec3f,
            atomic::Atomic,
            clump::Clump,
            frame::Frame,
            geometry::{Geometry, GeometryMorphTarget},
        };
        let vector = |x, y, z| Vec3f { x, y, z };
        let frame = |parent, x, y| Frame {
            right: vector(1., 0., 0.),
            up: vector(0., 1., 0.),
            at: vector(0., 0., 1.),
            pos: vector(x, y, 0.),
            parent,
            matrix_flags: 0,
            extensions: vec![],
        };
        let geometry = |y, radius| Geometry {
            flags: FormatFlag(0),
            vertices_count: 0,
            prelit: None,
            texcoord_sets: vec![],
            triangles: vec![],
            materials: vec![],
            morph_targets: vec![GeometryMorphTarget {
                bounding_sphere_x: 0.,
                bounding_sphere_y: y,
                bounding_sphere_z: 0.,
                radius,
                vertices: None,
                normals: None,
            }],
            extensions: vec![],
        };
        let atomic = |frame, geometry| Atomic {
            frame,
            geometry,
            unknown: 0,
            unknown2: 0,
            extensions: vec![],
        };
        let mut parent = frame(0, 0., 6.);
        parent.right = vector(0., 1., 0.);
        parent.up = vector(-1., 0., 0.);
        let mut chunks = [Clump {
            header: ChunkHeader {
                ty: ChunkType::CLUMP,
                length: 0,
                build_number: 0,
                version: 0,
            },
            light_count: 0,
            camera_count: 0,
            frames: vec![frame(-1, 1000., 2000.), parent, frame(1, 2., 0.)],
            geometries: vec![geometry(0., 2.), geometry(0., 2.), geometry(4., 1.)],
            atomics: vec![atomic(0, 0), atomic(2, 1), atomic(0, 2)],
            extensions: vec![],
        }];
        // Two radius-2 spheres at Y=0 and Y=8 merge to radius 6;
        // the third sphere is contained in that result.
        assert_eq!(
            AssetLoader::compute_actor_interaction_diameter(&chunks),
            12.
        );
        chunks[0].frames[0].right.x = 7.;
        chunks[0].frames[0].up.y = 7.;
        assert_eq!(
            AssetLoader::compute_actor_interaction_diameter(&chunks),
            12.
        );
        chunks[0].geometries[2].morph_targets[0].radius = 8.;
        assert_eq!(
            AssetLoader::compute_actor_interaction_diameter(&chunks),
            16.
        );
        chunks[0].geometries[2].morph_targets[0].radius = f32::NAN;
        assert_eq!(
            AssetLoader::compute_actor_interaction_diameter(&chunks),
            12.
        );
        chunks[0].geometries = vec![geometry(0., 1.), geometry(0., 1.), geometry(6., 1.)];
        chunks[0].geometries[1].morph_targets[0].bounding_sphere_x = 8.;
        chunks[0].atomics = vec![atomic(0, 0), atomic(0, 1), atomic(0, 2)];
        // Native reverse traversal contains the last sphere after merging
        // the first two; serialized-order traversal would exceed diameter 13.
        assert_eq!(
            AssetLoader::compute_actor_interaction_diameter(&chunks),
            12.
        );
        chunks[0].geometries = vec![geometry(0., 3.), geometry(0., 1.)];
        chunks[0].atomics = vec![atomic(0, 0), atomic(0, 1)];
        assert_eq!(AssetLoader::compute_actor_interaction_diameter(&chunks), 6.);
    }

    #[test]
    fn physical_interaction_finds_metadata_pickups_and_skips_hidden_or_claimed_objects() {
        use radiance::{
            comdef::IComponent,
            input::{Axis, AxisState, InputEngine, Key, KeyState, SyntheticInputBridge},
            math::{Mat44, Vec3},
            scene::CoreEntity,
        };
        struct Input;
        impl InputEngine for Input {
            fn get_key_state(&self, _: Key) -> KeyState {
                KeyState::default()
            }
            fn get_axis_state(&self, _: Axis) -> AxisState {
                AxisState::default()
            }
        }
        let mut scene = super::super::scene::Pal4Scene::new_empty();
        scene.player_interaction_diameters[0] = 80.;
        let mut entry = pickup();
        entry
            .properties
            .push(GobProperty::GobPropertyI32(GobPropertyI32 {
                ty: 1,
                name: GobFixedProperty::HEIGHT_LIMIT.into(),
                value: 1,
            }));
        let entity = CoreEntity::create("item001".into(), true);
        entity.add_tag(TAG_OBJECT);
        entity.add_component(
            super::super::comdef::IPal4ObjectComponent::uuid(),
            Pal4ObjectComponent::create(entry, GobObjectType::GET_ITEM, Mat44::new_identity())
                .query_interface::<IComponent>()
                .unwrap(),
        );
        scene.scene.add_entity(entity.clone());
        let input = || {
            let bridge = SyntheticInputBridge::new(std::rc::Rc::new(RefCell::new(Input)));
            bridge.tap(Key::F);
            std::rc::Rc::new(RefCell::new(bridge)) as std::rc::Rc<RefCell<dyn InputEngine>>
        };
        assert!(matches!(
            scene.test_interaction(input(), 0),
            Some(super::super::scene::SceneInteraction::Reward(_))
        ));
        entity.set_visible(false);
        assert!(scene.test_interaction(input(), 0).is_none());
        entity.set_visible(true);
        entity.set_enabled(false);
        assert!(scene.test_interaction(input(), 0).is_none());
        entity.set_enabled(true);
        entity
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(60., 0., 0.));
        entity.update_world_transform(&radiance::math::Transform::new());
        assert!(matches!(
            scene.test_interaction(input(), 0),
            Some(super::super::scene::SceneInteraction::Reward(_))
        ));
        entity
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(61., 0., 0.));
        entity.update_world_transform(&radiance::math::Transform::new());
        assert!(scene.test_interaction(input(), 0).is_none());
        entity
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(0., 0., 0.));
        entity.update_world_transform(&radiance::math::Transform::new());
        for height in [-80., 80., -81., 81.] {
            entity
                .transform()
                .borrow_mut()
                .set_position(&Vec3::new(0., height, 0.));
            entity.update_world_transform(&radiance::math::Transform::new());
            assert!(scene.test_interaction(input(), 0).is_none());
        }
        for height in [-79., 79., 0.] {
            entity
                .transform()
                .borrow_mut()
                .set_position(&Vec3::new(0., height, 0.));
            entity.update_world_transform(&radiance::math::Transform::new());
            assert!(matches!(
                scene.test_interaction(input(), 0),
                Some(super::super::scene::SceneInteraction::Reward(_))
            ));
        }
        let component = object_component(&entity).unwrap();
        let mut rewards = component
            .inner::<Pal4ObjectComponent>()
            .rewards
            .as_ref()
            .unwrap()
            .borrow_mut();
        assert_eq!(rewards.start(0), Ok(true));
        drop(rewards);
        assert!(scene.test_interaction(input(), 0).is_none());
        let unbounded = CoreEntity::create("item002".into(), true);
        unbounded.add_tag(TAG_OBJECT);
        unbounded.add_component(
            super::super::comdef::IPal4ObjectComponent::uuid(),
            Pal4ObjectComponent::create(pickup(), GobObjectType::GET_ITEM, Mat44::new_identity())
                .query_interface::<IComponent>()
                .unwrap(),
        );
        unbounded
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(0., 1000., 0.));
        unbounded.update_world_transform(&radiance::math::Transform::new());
        scene.scene.add_entity(unbounded);
        assert!(matches!(
            scene.test_interaction(input(), 0),
            Some(super::super::scene::SceneInteraction::Reward(_))
        ));
    }

    #[test]
    fn silent_signed_properties_and_capped_notices_use_requested_quantity() {
        let catalog = catalog();
        let mut state = Pal4PersistentState::new(PAL4_APP_NAME.into());
        assert_eq!(
            Reward::Property(3000, 98, false)
                .apply(&mut state, &catalog, false)
                .unwrap(),
            (None, None)
        );
        let (text, sound) = Reward::Property(3000, 5, false)
            .apply(&mut state, &catalog, true)
            .unwrap();
        assert_eq!(state.equipment_count(3000), 99);
        assert_eq!(text.as_deref(), Some("gain medicine x5"));
        assert_eq!(sound, Some("WA06.wav"));
        let (text, _) = Reward::Property(3000, -3, false)
            .apply(&mut state, &catalog, true)
            .unwrap();
        assert_eq!(state.equipment_count(3000), 96);
        assert_eq!(text.as_deref(), Some("gain medicine x-3"));
        let (text, sound) = Reward::Property(3000, -2, true)
            .apply(&mut state, &catalog, true)
            .unwrap();
        assert_eq!(state.equipment_count(3000), 98);
        assert_eq!(text.as_deref(), Some("lose medicine x-2"));
        assert_eq!(sound, Some("WA07.wav"));
        assert!(
            Reward::Property(9999, 1, false)
                .apply(&mut state, &catalog, true)
                .is_err()
        );
    }

    #[test]
    fn equipment_grants_one_instance_and_only_owned_removals_notify() {
        let catalog = catalog();
        let mut state = Pal4PersistentState::new(PAL4_APP_NAME.into());
        let (text, _) = Reward::Equipment(3500, Some(5), false)
            .apply(&mut state, &catalog, true)
            .unwrap();
        assert_eq!(state.equipment_count(3500), 1);
        assert_eq!(text.as_deref(), Some("gain sword x5"));
        let (text, sound) = Reward::Equipment(3500, None, true)
            .apply(&mut state, &catalog, true)
            .unwrap();
        assert_eq!(text.as_deref(), Some("lose sword"));
        assert_eq!(sound, Some("WA07.wav"));
        assert_eq!(
            Reward::Equipment(3500, None, true)
                .apply(&mut state, &catalog, true)
                .unwrap(),
            (None, None)
        );
        assert_eq!(
            Reward::Equipment(3500, None, false)
                .apply(&mut state, &catalog, true)
                .unwrap()
                .0
                .as_deref(),
            Some("gain sword x1")
        );
    }

    #[test]
    fn signed_money_clamps_and_silent_acquisition_still_plays_native_sound() {
        let catalog = catalog();
        let mut state = Pal4PersistentState::new(PAL4_APP_NAME.into());
        assert_eq!(
            Reward::Money(i32::MAX, false)
                .apply(&mut state, &catalog, false)
                .unwrap(),
            (None, Some("WA40.wav"))
        );
        assert_eq!(state.money(), 9_999_999);
        Reward::Money(i32::MAX, true)
            .apply(&mut state, &catalog, true)
            .unwrap();
        assert_eq!(state.money(), 0);
        let (text, sound) = Reward::Money(-5, true)
            .apply(&mut state, &catalog, true)
            .unwrap();
        assert_eq!(state.money(), 5);
        assert_eq!(text.as_deref(), Some("pay money -5"));
        assert_eq!(sound, None);
        Reward::Money(i32::MIN, false)
            .apply(&mut state, &catalog, false)
            .unwrap();
        assert_eq!(state.money(), 0);
    }

    #[test]
    fn metadata_pickup_claim_and_pending_chest_cursor_survive_serialization() {
        let entry = pickup();
        let rewards = ObjectRewards::from_entry(&entry, GobObjectType::GET_ITEM).unwrap();
        let mut rewards = rewards.borrow_mut();
        assert_eq!(rewards.start(0), Ok(true));
        assert_eq!(rewards.start(0), Ok(false));
        assert!(matches!(
            rewards.plan.tasks[0],
            RewardTask::Grant(Reward::Property(3000, 5, false))
        ));
        let mut state = Pal4PersistentState::new(PAL4_APP_NAME.into());
        state.set_object_reward("q01\\q01\\item001".into(), rewards.progress.clone());
        let restored: Pal4PersistentState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(
            restored.object_reward("q01\\q01\\item001"),
            Some(&RewardProgress {
                remaining: 0,
                next_task: Some(0)
            })
        );
        rewards.progress.next_task = None;
        assert!(!rewards.available());
        let mut unstored = entry.clone();
        if let GobProperty::GobPropertyI32(property) = &mut unstored.properties[0] {
            property.value = 0;
        }
        assert!(
            !ObjectRewards::from_entry(&unstored, GobObjectType::GET_ITEM)
                .unwrap()
                .borrow()
                .plan
                .stored
        );
    }

    #[test]
    fn reward_notices_replace_reset_lifetime_and_do_not_restore_with_progress() {
        let mut session = Pal4Session::new();
        session.show_reward_notice("first".into());
        session.advance_reward_notice(0.75);
        session.show_reward_notice("second".into());
        session.advance_reward_notice(0.75);
        assert_eq!(session.reward_notice(), Some("second"));
        session.advance_reward_notice(0.25);
        assert!(session.reward_notice().is_none());
        session.show_reward_notice("old".into());
        session.set_reward_busy(true);
        let restored: Pal4PersistentState =
            serde_json::from_str(&serde_json::to_string(session.state()).unwrap()).unwrap();
        let loaded = Pal4Session::from_state(restored);
        assert!(loaded.reward_notice().is_none());
        assert!(!loaded.reward_busy());
    }

    #[test]
    fn actor_restrictions_reject_without_claiming_and_accept_the_native_leader() {
        for (valid_for, template) in [(1, 265), (3, 266)] {
            let mut entry = pickup();
            entry
                .properties
                .push(GobProperty::GobPropertyI32(GobPropertyI32 {
                    ty: 1,
                    name: GobFixedProperty::VALIDFOR.into(),
                    value: valid_for,
                }));
            let rewards = ObjectRewards::from_entry(&entry, GobObjectType::GET_ITEM).unwrap();
            let mut rewards = rewards.borrow_mut();
            let progress = rewards.progress.clone();
            for leader in 0..4 {
                if leader != valid_for as usize {
                    assert_eq!(rewards.start(leader), Err(template));
                    assert_eq!(rewards.progress, progress);
                }
            }
            assert_eq!(rewards.start(valid_for as usize), Ok(true));
            assert_eq!(rewards.start(0), Ok(false));
        }
        for valid_for in [-1, 0, 2, 4] {
            let mut entry = pickup();
            entry
                .properties
                .push(GobProperty::GobPropertyI32(GobPropertyI32 {
                    ty: 1,
                    name: GobFixedProperty::VALIDFOR.into(),
                    value: valid_for,
                }));
            let rewards = ObjectRewards::from_entry(&entry, GobObjectType::GET_ITEM).unwrap();
            assert_eq!(rewards.borrow_mut().start(3), Ok(true));
        }
    }

    #[test]
    #[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
    fn original_actor_specific_pickups_preserve_rewards_until_ziying_collects() {
        use binrw::BinRead;
        let root = std::env::var("PAL4_ASSET_ROOT").unwrap();
        let vfs = packfs::init_virtual_fs(&root, None);
        let catalog = GameplayCatalog::load(&vfs).unwrap();
        for actor in ["101", "103", "106", "105"] {
            let bytes = vfs
                .read_to_end(&format!("/gamedata/PALActor/{actor}/{actor}.dff"))
                .unwrap();
            let chunks = fileformats::rwbs::read_dff(&bytes).unwrap();
            let diameter =
                super::super::asset_loader::AssetLoader::compute_actor_interaction_diameter(
                    &chunks,
                );
            assert!(diameter.is_finite() && diameter > 0., "{actor}: {diameter}");
            eprintln!("Native model interaction diameter {actor}: {diameter}");
        }
        for template in [265, 266] {
            assert!(!catalog.game_strings[&template].is_empty());
            eprintln!(
                "Restriction notice {template}: {}",
                catalog.game_strings[&template]
            );
        }
        let mut directories = vec![std::path::PathBuf::from("/gamedata/scenedata")];
        let mut visited = std::collections::HashSet::new();
        let mut count = 0;
        while let Some(directory) = directories.pop() {
            for entry in mini_fs::Store::entries_path(&vfs, &directory).unwrap() {
                let entry = entry.unwrap();
                let child = directory.join(std::path::Path::new(&entry.name).file_name().unwrap());
                if !visited.insert(child.to_string_lossy().to_ascii_lowercase()) {
                    continue;
                }
                if matches!(entry.kind, mini_fs::EntryKind::Dir) {
                    directories.push(child);
                    continue;
                }
                if !child
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("GameObjs.gob")
                {
                    continue;
                }
                let bytes = vfs.read_to_end(&child).unwrap();
                let gob = fileformats::pal4::gob::GobFile::read(&mut std::io::Cursor::new(bytes))
                    .unwrap();
                for (entry, kind) in gob.entries.iter().zip(gob.header.object_types) {
                    if kind != GobObjectType::GET_ITEM
                        || entry
                            .get_property(GobFixedProperty::VALIDFOR)
                            .and_then(GobProperty::value_i32)
                            != Some(3)
                    {
                        continue;
                    }
                    let rewards = ObjectRewards::from_entry(entry, kind).unwrap();
                    let mut rewards = rewards.borrow_mut();
                    let progress = rewards.progress.clone();
                    for leader in 0..3 {
                        assert_eq!(rewards.start(leader), Err(266));
                        assert_eq!(rewards.progress, progress);
                    }
                    assert_eq!(rewards.start(3), Ok(true));
                    let mut state = Pal4PersistentState::new(PAL4_APP_NAME.into());
                    for task in &rewards.plan.tasks {
                        if let RewardTask::Grant(reward) = task {
                            reward.apply(&mut state, &catalog, true).unwrap();
                        }
                    }
                    if count == 0 {
                        eprintln!(
                            "Representative Ziying pickup: {} {}",
                            child.display(),
                            entry.name.to_string().unwrap()
                        );
                    }
                    count += 1;
                }
            }
        }
        assert!(count > 0);
        eprintln!("Verified {count} original actor-specific pickups");
    }

    #[test]
    #[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
    fn original_pickups_chests_and_hint_artwork_use_native_metadata() {
        use binrw::BinRead;
        let root = std::env::var("PAL4_ASSET_ROOT").unwrap();
        let vfs = packfs::init_virtual_fs(&root, None);
        let catalog = GameplayCatalog::load(&vfs).unwrap();
        for (scene, block, name, property, count) in [
            ("Q01", "Q01", "item008", 3035, 1),
            ("M01", "1", "item001", 3000, 1),
        ] {
            let bytes = vfs
                .read_to_end(&format!("/gamedata/scenedata/{scene}/{block}/GameObjs.gob"))
                .unwrap();
            let gob =
                fileformats::pal4::gob::GobFile::read(&mut std::io::Cursor::new(bytes)).unwrap();
            let index = gob
                .entries
                .iter()
                .position(|entry| entry.name.to_string().unwrap() == name)
                .unwrap();
            let rewards =
                ObjectRewards::from_entry(&gob.entries[index], gob.header.object_types[index])
                    .unwrap();
            let rewards = rewards.borrow();
            assert!(
                matches!(rewards.plan.tasks[0], RewardTask::Grant(Reward::Property(id, amount, false))
                                if (id, amount) == (property, count))
            );
            assert!(rewards.plan.stored && rewards.plan.disappear);
        }
        let bytes = vfs
            .read_to_end("/gamedata/scenedata/Q01/Q01/GameObjs.gob")
            .unwrap();
        let gob = fileformats::pal4::gob::GobFile::read(&mut std::io::Cursor::new(bytes)).unwrap();
        let index = gob
            .entries
            .iter()
            .position(|entry| entry.name.to_string().unwrap() == "item101")
            .unwrap();
        let rewards =
            ObjectRewards::from_entry(&gob.entries[index], gob.header.object_types[index]).unwrap();
        let rewards = rewards.borrow();
        assert!(rewards.plan.chest && rewards.plan.stored && !rewards.plan.disappear);
        assert!(rewards.plan.opening_sound);
        assert!(
            !vfs.read_to_end("/gamedata/PALSound/WC003.wav")
                .unwrap()
                .is_empty()
        );
        assert!(
            matches!(&rewards.plan.tasks[0], RewardTask::Animation(action) if action == "OM07")
        );
        assert_eq!(rewards.plan.tasks.len(), 4);
        let mut state = Pal4PersistentState::new(PAL4_APP_NAME.into());
        for task in &rewards.plan.tasks {
            if let RewardTask::Grant(reward) = task {
                reward.apply(&mut state, &catalog, true).unwrap();
            }
        }
        assert_eq!(
            (
                state.equipment_count(3558),
                state.equipment_count(3648),
                state.equipment_count(3182)
            ),
            (1, 1, 3)
        );
        let layout =
            crate::loaders::cegui::layout::load_layout(&vfs, "/gamedata/ui/layouts/hintWindow.xml")
                .unwrap();
        assert!(
            layout
                .windows
                .iter()
                .any(|window| window.name == "ImgFrame")
        );
        assert!(
            layout
                .windows
                .iter()
                .any(|window| window.name == "EditMessage")
        );
        assert!(catalog.game_strings[&251].starts_with("获得"));
        assert!(catalog.game_strings[&252].starts_with("失去"));
        let mut index = crate::loaders::cegui::ui_layout_handle::ImagesetIndex::new(
            "/gamedata/ui/imagesets".into(),
        );
        for window in layout.windows {
            for property in window.properties {
                if property.name.ends_with("Image") {
                    if let Some(reference) =
                        crate::loaders::cegui::layout::parse_image_ref(&property.value)
                    {
                        let (atlas, _) = index.load(&vfs, &reference.set).unwrap();
                        assert!(atlas.get(&reference.image).is_some(), "{}", reference.image);
                    }
                }
            }
        }
    }
}
impl Pal4VmContext {
    pub(crate) fn start_object_reward(&mut self, entity: ComRc<IEntity>) -> bool {
        if self.active_reward.is_some()
            || self.is_player_locked()
            || self.session_handle().borrow().player_input_blocked()
            || !entity.enabled()
            || !entity.visible()
        {
            return false;
        }
        let Some(component) = object_component(&entity) else {
            return false;
        };
        let component = component.inner::<Pal4ObjectComponent>();
        if component.entry().active == 0 {
            return false;
        }
        let Some(rewards) = component.rewards.as_ref() else {
            return false;
        };
        let leader = self.session_handle().borrow().state().leader();
        let started = rewards.borrow_mut().start(leader);
        match started {
            Ok(true) => {}
            Ok(false) => return false,
            Err(template) => {
                let handle = self.session_handle();
                let text = handle
                    .borrow()
                    .catalog()
                    .game_strings
                    .get(&template)
                    .cloned();
                if let Some(text) = text.filter(|text| !text.is_empty()) {
                    let _ = self.loader.dialog_artwork();
                    handle.borrow_mut().show_reward_notice(text);
                }
                if let Err(error) = self.play_sound("WA05") {
                    log::warn!("PAL4 reward restriction sound: {error:#}");
                }
                return false;
            }
        }
        self.persist_object_reward(&entity);
        let chest = rewards.borrow().plan.chest;
        self.session_handle().borrow().set_reward_busy(chest);
        self.active_reward = Some(entity);
        self.tick_object_reward();
        true
    }

    fn reward_key(&self, entity: &ComRc<IEntity>) -> String {
        let handle = self.session_handle();
        let session = handle.borrow();
        let state = session.state();
        let block = if state.sub_block_name().is_empty() {
            state.block_name()
        } else {
            state.sub_block_name()
        };
        format!("{}\\{}\\{}", state.scene_name(), block, entity.name()).to_lowercase()
    }

    fn persist_object_reward(&self, entity: &ComRc<IEntity>) {
        let key = self.reward_key(entity);
        let component = object_component(entity).unwrap();
        let rewards = component
            .inner::<Pal4ObjectComponent>()
            .rewards
            .as_ref()
            .unwrap()
            .borrow();
        if rewards.plan.stored {
            self.session_handle()
                .borrow_mut()
                .state_mut()
                .set_object_reward(key, rewards.progress.clone());
        }
    }

    pub(crate) fn restore_object_rewards(&mut self) {
        self.active_reward = None;
        self.session_handle().borrow_mut().clear_reward_feedback();
        let entities = self.scene.borrow().scene.find_entities_by_tag(TAG_OBJECT);
        for entity in entities {
            let Some(component) = object_component(&entity) else {
                continue;
            };
            let component = component.inner::<Pal4ObjectComponent>();
            let Some(rewards) = component.rewards.as_ref() else {
                continue;
            };
            let restored = self
                .session_handle()
                .borrow()
                .state()
                .object_reward(&self.reward_key(&entity))
                .cloned();
            let Some(restored) = restored else { continue };
            let mut rewards = rewards.borrow_mut();
            rewards.progress = restored;
            if rewards.progress.next_task.is_some() {
                self.active_reward = Some(entity.clone());
                self.session_handle()
                    .borrow()
                    .set_reward_busy(rewards.plan.chest);
            }
            if rewards.progress.remaining == 0
                && rewards.progress.next_task.is_none()
                && rewards.plan.disappear
            {
                entity.set_visible(false);
                entity.set_enabled(false);
            } else {
                // Completed animation tasks restore their final pose without replaying sounds.
                let completed = rewards
                    .progress
                    .next_task
                    .unwrap_or(rewards.plan.tasks.len());
                if let Some(RewardTask::Animation(action)) = rewards.plan.tasks
                    [..completed.min(rewards.plan.tasks.len())]
                    .iter()
                    .rev()
                    .find(|task| matches!(task, RewardTask::Animation(_)))
                {
                    if let Ok(mut animation) = self
                        .loader
                        .load_object_animation(&component.folder(), action)
                    {
                        for track in &mut animation.keyframes {
                            if let Some(mut frame) = track.last().cloned() {
                                frame.timestamp = 0.;
                                *track = vec![frame];
                            }
                        }
                        play_object_animation(&entity, animation.keyframes, vec![], false, true);
                    }
                }
            }
        }
    }

    pub(crate) fn tick_object_reward(&mut self) {
        let Some(entity) = self.active_reward.clone() else {
            return;
        };
        let component = object_component(&entity).unwrap();
        let component = component.inner::<Pal4ObjectComponent>();
        let rewards_cell = component.rewards.as_ref().unwrap();
        let mut rewards = rewards_cell.borrow_mut();
        if rewards.waiting_animation {
            if entity.enabled()
                && object_armature(&entity)
                    .is_some_and(|armature| armature.animation_state() == AnimationState::Playing)
                && !self.fast_forward()
            {
                return;
            }
            rewards.waiting_animation = false;
            rewards.progress.next_task = rewards.progress.next_task.map(|index| index + 1);
            drop(rewards);
            self.persist_object_reward(&entity);
            rewards = rewards_cell.borrow_mut();
        }
        if rewards.plan.chest && self.session_handle().borrow().reward_notice().is_some() {
            return;
        }
        let index = rewards
            .progress
            .next_task
            .unwrap_or(rewards.plan.tasks.len());
        match rewards.plan.tasks.get(index).cloned() {
            Some(RewardTask::Animation(action)) => {
                if rewards.plan.opening_sound {
                    if let Err(error) = self.play_sound("WC003") {
                        log::warn!("PAL4 chest opening sound: {error:#}");
                    }
                }
                let animation = self
                    .loader
                    .load_object_animation(&component.folder(), &action);
                let started = animation
                    .map(|animation| {
                        play_object_animation(
                            &entity,
                            animation.keyframes,
                            animation.events,
                            false,
                            true,
                        )
                    })
                    .unwrap_or_else(|error| {
                        log::warn!("PAL4 chest animation: {error:#}");
                        false
                    });
                if started {
                    rewards.waiting_animation = true;
                } else {
                    rewards.progress.next_task = Some(index + 1);
                }
            }
            Some(RewardTask::Grant(reward)) => {
                // The grant and its cursor share one session borrow: snapshots cannot duplicate it.
                let handle = self.session_handle();
                let key = self.reward_key(&entity);
                let mut session = handle.borrow_mut();
                let catalog = session.catalog();
                let result = reward.apply(session.state_mut(), &catalog, true);
                rewards.progress.next_task = Some(index + 1);
                if rewards.plan.stored {
                    session
                        .state_mut()
                        .set_object_reward(key, rewards.progress.clone());
                }
                drop(session);
                match result {
                    Ok((text, sound)) => {
                        if let Some(text) = text {
                            let _ = self.loader.dialog_artwork();
                            handle.borrow_mut().show_reward_notice(text);
                        }
                        if let Some(sound) = sound {
                            if let Err(error) = self.play_sound(sound.trim_end_matches(".wav")) {
                                log::warn!("PAL4 reward sound: {error:#}");
                            }
                        }
                    }
                    Err(error) => log::warn!("PAL4 object reward: {error:#}"),
                }
                if !rewards.plan.chest {
                    rewards.progress.next_task = None;
                }
            }
            None => rewards.progress.next_task = None,
        }
        let finished = rewards.progress.next_task.is_none();
        if finished && rewards.progress.remaining == 0 && rewards.plan.disappear {
            entity.set_visible(false);
            entity.set_enabled(false);
        }
        drop(rewards);
        self.persist_object_reward(&entity);
        if finished {
            self.active_reward = None;
            self.session_handle().borrow().set_reward_busy(false);
        }
    }
}
