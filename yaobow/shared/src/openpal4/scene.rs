use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crosscom::ComRc;
use fileformats::npc::NpcInfoFile;
use fileformats::pal4::{
    evf::EvfEvent,
    gob::{GobCommonProperties, GobFile, GobObjectType},
};
use radiance::{
    comdef::{
        ICameraControl, IEntity, IEntityExt, IRayCaster, IScene, ISceneExt, IStaticMeshComponent,
    },
    input::InputEngine,
    math::{Mat44, Transform, Vec3},
    rendering::GradientYMaterialDef,
    scene::{wrap_scene_camera, CoreEntity, CoreScene},
    utils::ray_casting::{wrap_ray_caster, RayCaster},
};
use radiance_scripting::{comdef::services::IInputService, services::InputService};

use crate::scripting::angelscript::ScriptModule;

use super::{
    asset_loader::{self, AssetLoader},
    comdef::{IPal4ActorAnimationController, IPal4ActorController, IPal4GameContext},
    game_context::Pal4GameContext,
    uv_anim::UvAnimDriver,
};

/// Factory abstraction supplied by the runtime (yaobow) at PAL4 boot.
/// Mints a single party-wide `IPal4ActorController` that receives all
/// four party-member entities + animation handles plus the shared
/// engine surface. The runtime attaches the returned component to a
/// synthetic "party root" entity that parents the four players, so a
/// single controller drives the active leader each frame instead of
/// four self-gating wrappers sharing scene state. Editor previews pass
/// `None` for the factory and the scene loads without any per-player
/// controller component attached.
pub trait Pal4ActorControllerFactory {
    fn make_actor_controller(
        &self,
        game_ctx: ComRc<IPal4GameContext>,
        input: ComRc<IInputService>,
        entities: [ComRc<IEntity>; 4],
        anims: [ComRc<IPal4ActorAnimationController>; 4],
        camera: ComRc<ICameraControl>,
        ray_caster: ComRc<IRayCaster>,
    ) -> ComRc<IPal4ActorController>;
}

pub enum Player {
    YunTianhe,
    HanLingsha,
    LiuMengli,
    MurongZiying,
}

impl Player {
    pub fn name(&self) -> &str {
        match self {
            Player::YunTianhe => "YunTianhe",
            Player::HanLingsha => "HanLingsha",
            Player::LiuMengli => "LiuMengli",
            Player::MurongZiying => "MurongZiying",
        }
    }

    pub fn actor_name(&self) -> &str {
        match self {
            Player::YunTianhe => "101",
            Player::HanLingsha => "103",
            Player::LiuMengli => "106",
            Player::MurongZiying => "105",
        }
    }
}

pub struct Pal4Scene {
    pub(crate) scene: ComRc<IScene>,
    pub(crate) players: [ComRc<IEntity>; 4],
    pub(crate) npcs: Vec<ComRc<IEntity>>,
    pub(crate) objects: Vec<ComRc<IEntity>>,
    /// World-space XZ axis-aligned bounding boxes for each entry in
    /// `objects`, computed once at scene load. `(min_x, max_x, min_z,
    /// max_z)`; `None` when the object's entity tree contains no
    /// static mesh (e.g. invisible spawn markers).
    ///
    /// Used by [`Pal4Scene::test_interaction`] to decide whether the
    /// player is "near" an interactable GOB. The naive point-distance
    /// check against `entry.position` is wrong for any GOB whose
    /// authored anchor is offset from the visible mesh — most
    /// notably the ladders in Q01/Q01, where the anchor sits in the
    /// middle of the slope but the mesh extends ~150 units in Y and
    /// ~250 units in XZ. After the climb-down handler teleports the
    /// player to the bottom of the slope, they end up 264 XZ units
    /// from the anchor — well beyond the 60-unit `trigger_distance`
    /// — even though they're physically right next to the visible
    /// ladder, so the F-key climb-up interaction would never fire.
    pub(crate) objects_xz_aabbs: Vec<Option<(f32, f32, f32, f32)>>,
    pub(crate) objects_gob: Option<GobFile>,
    pub(crate) events: Vec<EvfEvent>,
    pub(crate) module: Option<Rc<RefCell<ScriptModule>>>,
    pub(crate) triggers: Vec<Rc<SceneEventTrigger>>,
    pub(crate) uv_anim: UvAnimDriver,
    // Handles captured at load time so the PAL4 debug overlay can flip
    // their visibility at runtime. `bsp_entity` is the BSP "world" root
    // returned by `AssetLoader::load_scene`. `floor_entity` /
    // `wall_entity` are the per-block collision meshes that used to be
    // gated by compile-time `SHOW_FLOOR` / `SHOW_WALL` constants — they
    // are now always added to the scene but start hidden.
    pub(crate) bsp_entity: Option<ComRc<IEntity>>,
    pub(crate) floor_entity: Option<ComRc<IEntity>>,
    pub(crate) wall_entity: Option<ComRc<IEntity>>,
    /// Engine-owned `Pal4GameContext` CCW shared with the four
    /// scripted `Pal4ActorController` wrappers. `set_active_leader`
    /// writes through this so the actor controllers observe the new
    /// leader index via `IPal4GameContext::current_leader()`. `None`
    /// on the placeholder scene returned by `new_empty`.
    pub(crate) game_context: Option<ComRc<IPal4GameContext>>,
    /// Single party-wide actor controller component attached to the
    /// synthetic party-root entity that parents the four players.
    /// `None` on the placeholder scene returned by `new_empty` and
    /// when no `actor_controller_factory` was installed.
    pub(crate) actor_controller: Option<ComRc<IPal4ActorController>>,
}

const SHOW_TRIGGER_POINT: bool = false;

impl Pal4Scene {
    const ID_YUN_TIANHE: usize = 0;
    const ID_HAN_LINGSHA: usize = 1;
    const ID_LIU_MENGLI: usize = 2;
    const ID_MURONG_ZIYING: usize = 3;

    pub fn new_empty() -> Self {
        Self {
            scene: CoreScene::create(),
            players: [
                CoreEntity::create("".to_string(), false),
                CoreEntity::create("".to_string(), false),
                CoreEntity::create("".to_string(), false),
                CoreEntity::create("".to_string(), false),
            ],
            npcs: vec![],
            objects: vec![],
            objects_xz_aabbs: vec![],
            objects_gob: None,
            events: vec![],
            module: None,
            triggers: vec![],
            uv_anim: UvAnimDriver::new(),
            bsp_entity: None,
            floor_entity: None,
            wall_entity: None,
            game_context: None,
            actor_controller: None,
        }
    }

    /// Drive UV animation each frame for water (and any other entity
    /// registered with the driver). No-op when no animation is bound.
    pub fn tick_uv_anim(&mut self, delta_sec: f32) {
        self.uv_anim.tick(delta_sec);
    }

    pub fn load(
        asset_loader: &Rc<asset_loader::AssetLoader>,
        input: Rc<RefCell<dyn InputEngine>>,
        scene_name: &str,
        block_name: &str,
        actor_controller_factory: Option<&Rc<dyn Pal4ActorControllerFactory>>,
    ) -> anyhow::Result<Self> {
        let (scene, bsp_entity) = asset_loader.load_scene(scene_name, block_name)?;

        if !cfg!(vita) {
            let clip = asset_loader.try_load_scene_clip(scene_name, block_name);
            if let Some(clip) = clip {
                scene.add_entity(clip);
            }
        }

        let clip_na = asset_loader.try_load_scene_clip_na(scene_name, block_name);
        if let Some(clip_na) = clip_na {
            scene.add_entity(clip_na);
        }

        let skybox = asset_loader.try_load_scene_sky(scene_name, block_name);
        if let Some(skybox) = skybox {
            scene.add_entity(skybox);
        }

        // Optional water surface (PAL4 scenes that ship a `_water.dff`,
        // e.g. Q01/q01/Q01, Q01/q01/Q01Y). The sibling `_water.uva`
        // drives per-frame UV animation via `UvAnimDriver`.
        let mut uv_anim = UvAnimDriver::new();
        let water = asset_loader.try_load_scene_water(scene_name, block_name);
        if let Some(water) = water {
            scene.add_entity(water.clone());
            if let Some(dict) = asset_loader.try_load_scene_water_uva(scene_name, block_name) {
                log::debug!(
                    "Loaded water UV-anim dict for {}/{}: {} animation(s)",
                    scene_name,
                    block_name,
                    dict.animations.len()
                );
                uv_anim.register_water_entity(water, &dict);
            }
        }

        scene.camera_mut().set_fov43(45_f32.to_radians());

        let floor = asset_loader.load_scene_floor(scene_name, block_name);
        let wall = asset_loader.load_scene_wall(scene_name, block_name);
        if floor.is_none() {
            log::warn!(
                "Pal4Scene::load: missing floor mesh for scene='{}' block='{}'. \
                 Floor collision raycast will be empty for this block; the \
                 active leader may freeze in place or fall through geometry.",
                scene_name,
                block_name
            );
        }
        if wall.is_none() {
            log::warn!(
                "Pal4Scene::load: missing wall mesh for scene='{}' block='{}'. \
                 Wall collision raycast will be empty for this block; the \
                 active leader may walk through walls.",
                scene_name,
                block_name
            );
        }
        let ray_caster = create_floor_wall_ray_caster(floor.clone(), wall.clone());

        // Compute the union world-Y range across floor + wall geometry,
        // then replace each `Geometry.material` with a
        // `GradientYMaterialDef` so when the PAL4 debug overlay reveals
        // the nav-mesh it renders as a blue-(low)→red-(high) vertical
        // heatmap. Must happen before `scene.add_entity` because the
        // entity's `StaticMeshComponent::on_loading` (fired during the
        // add) snapshots `Geometry.material` into the render objects.
        let mut y_lo = f32::INFINITY;
        let mut y_hi = f32::NEG_INFINITY;
        for entity_opt in [floor.as_ref(), wall.as_ref()].iter().copied() {
            if let Some(e) = entity_opt {
                if let Some((lo, hi)) = entity_world_y_range(e) {
                    y_lo = y_lo.min(lo);
                    y_hi = y_hi.max(hi);
                }
            }
        }
        if y_lo.is_finite() && y_hi.is_finite() && y_hi > y_lo {
            for entity_opt in [floor.as_ref(), wall.as_ref()].iter().copied() {
                if let Some(e) = entity_opt {
                    apply_gradient_material(e, y_lo, y_hi);
                }
            }
        }

        // Always add floor + wall so the PAL4 debug overlay can toggle
        // them on at runtime. They default to hidden — matches the old
        // SHOW_FLOOR / SHOW_WALL = false behaviour.
        if let Some(floor) = floor.as_ref() {
            floor.set_visible(false);
            floor.set_enabled(false);
            scene.add_entity(floor.clone());
        }

        if let Some(wall) = wall.as_ref() {
            wall.set_visible(false);
            wall.set_enabled(false);
            scene.add_entity(wall.clone());
        }

        let players = [
            load_player(asset_loader, Player::YunTianhe),
            load_player(asset_loader, Player::HanLingsha),
            load_player(asset_loader, Player::LiuMengli),
            load_player(asset_loader, Player::MurongZiying),
        ];

        let events = asset_loader.load_evf(scene_name, block_name)?;

        let mut triggers = vec![];
        for (i, event) in events.events.iter().enumerate() {
            let trigger = event
                .vertices
                .iter()
                .map(|trigger| {
                    Vec3::new(
                        trigger.center.x as f32,
                        trigger.center.y as f32,
                        trigger.center.z as f32,
                    )
                })
                .collect::<Vec<_>>();

            if SHOW_TRIGGER_POINT {
                for point in &trigger {
                    let entity =
                        radiance::debug::create_box_entity(asset_loader.component_factory());
                    entity.transform().borrow_mut().set_position(point);
                    scene.add_entity(entity);
                }
            }

            if event.vertex_count != 8 && event.vertex_count != 4 {
                continue;
            }

            let ray_caster = create_trigger_ray_caster(trigger);
            triggers.push(Rc::new(SceneEventTrigger {
                ray_caster,
                event_index: i,
                triggered: Cell::new(false),
            }));
        }

        // Build engine-side scriptable handles once per scene; share
        // them across all four actor controllers. The same `ray_caster`
        // backs both the per-frame floor/wall probes (via `IRayCaster`)
        // and `IPal4GameContext::check_event_triggers` could in
        // principle reach it too, but triggers carry their own caster
        // per `SceneEventTrigger`.
        let ray_caster_rc = Rc::new(ray_caster);
        let game_context = Pal4GameContext::create(triggers.clone());

        let actor_controller = if let Some(factory) = actor_controller_factory {
            let input_service = InputService::create(input.clone());
            let camera_ctrl = wrap_scene_camera(scene.clone());
            let ray_caster_wrapped = wrap_ray_caster(ray_caster_rc.clone());
            let anims: [ComRc<IPal4ActorAnimationController>; 4] = std::array::from_fn(|i| {
                players[i]
                    .get_component(IPal4ActorAnimationController::uuid())
                    .and_then(|c| c.query_interface::<IPal4ActorAnimationController>())
                    .expect("player must carry an IPal4ActorAnimationController component")
            });
            let controller = factory.make_actor_controller(
                game_context.clone(),
                input_service.clone(),
                players.clone(),
                anims,
                camera_ctrl.clone(),
                ray_caster_wrapped.clone(),
            );
            let component = controller
                .query_interface::<radiance::comdef::IComponent>()
                .expect("scripted Pal4PartyController must QI to IComponent");

            // Party root entity owns the four players as children and
            // hosts the single controller component.
            let party_root = CoreEntity::create("PartyRoot".to_string(), true);
            for p in &players {
                party_root.attach(p.clone());
            }
            party_root.add_component(IPal4ActorController::uuid(), component);
            scene.add_entity(party_root);
            Some(controller)
        } else {
            for p in &players {
                scene.add_entity(p.clone());
            }
            None
        };

        // `npcInfo.npc` is optional on disk — some PAL4 blocks don't
        // ship one (e.g. `scenedata/M02/3/` has no `npcInfo.npc`).
        // Treat a missing/unreadable file as "no NPCs" instead of
        // failing the whole scene load, which would `?` out of
        // `giArenaLoad` and abort the surrounding cutscene.
        let npc_info = match asset_loader.load_npc_info(scene_name, block_name) {
            Ok(info) => info,
            Err(e) => {
                log::warn!(
                    "Pal4Scene::load: npcInfo.npc missing/unreadable for \
                     scene='{}' block='{}' ({:#}); proceeding with no NPCs",
                    scene_name,
                    block_name,
                    e
                );
                NpcInfoFile::default()
            }
        };
        let mut npcs = vec![];
        for npc in &npc_info.data {
            let actor_name = npc.model_name.to_string();
            match actor_name {
                Ok(actor_name) => {
                    let entity = asset_loader.load_actor(
                        npc.name.to_string().unwrap_or_default().as_str(),
                        actor_name.as_str(),
                        npc.get_default_act().as_deref(),
                    );

                    if let Ok(entity) = entity {
                        entity.set_visible(npc.default_visible == 1);
                        entity.set_enabled(npc.default_visible == 1);
                        entity
                            .transform()
                            .borrow_mut()
                            .set_position(&Vec3::new_zeros())
                            .rotate_axis_angle_local(&Vec3::BACK, npc.rotation[2].to_radians())
                            .rotate_axis_angle_local(&Vec3::UP, npc.rotation[1].to_radians())
                            .rotate_axis_angle_local(&Vec3::EAST, npc.rotation[0].to_radians())
                            .set_position(&Vec3::from(npc.position));

                        npcs.push(entity.clone());

                        scene.add_entity(entity);
                    }
                }
                Err(e) => {
                    log::error!("Cannot load actor: {}", e)
                }
            }
        }

        let mut objects = vec![];
        let gob = asset_loader.load_gob(scene_name, block_name)?;

        for (i, entry) in gob.entries.iter().enumerate() {
            let object_type = gob.header.object_types[i];
            let object_name = entry.file_name.to_string();
            let folder = entry.folder.to_string();
            let file_name = entry.file_name.to_string();
            // Scripts address game objects by the GOB entry's logical `name`
            // (e.g. via `giSetObjectVisible`), not by the mesh file name. Use
            // it for the entity name so `Pal4Scene::get_object` can find it,
            // mirroring how NPCs are named by `npc.name`. Fall back to the
            // file name if the logical name is missing.
            let logical_name = entry.name.to_string().ok();
            match (object_name, folder, file_name) {
                (Ok(object_name), Ok(folder), Ok(file_name)) => {
                    if object_type == GobObjectType::EFFECT {
                        continue;
                    }

                    let entity_name = logical_name.unwrap_or_else(|| object_name.clone());
                    let entity = asset_loader
                        .load_object(&entity_name, &folder, &file_name)
                        .unwrap_or_else(|| {
                            log::error!(
                                "Cannot load object: {:?} {:?} {:?}",
                                entity_name,
                                folder,
                                file_name
                            );
                            CoreEntity::create(entity_name.clone(), false)
                        });

                    // Honor the `PAL4-GameObject-object-hide` flag: when set,
                    // the entity is placed in the scene but starts invisible.
                    // Scripts can later reveal it via the `giGOB*` API.
                    let initially_hidden = entry.is_initially_hidden();
                    entity.set_visible(!initially_hidden);
                    entity.set_enabled(true);

                    let scale = entry
                        .get_common_property(GobCommonProperties::Scale)
                        .and_then(|s| s.value_f32())
                        .unwrap_or(1.0);

                    entity
                        .transform()
                        .borrow_mut()
                        .scale_local(&Vec3::new(scale, scale, scale))
                        .rotate_axis_angle_local(&Vec3::BACK, entry.rotation[2].to_radians())
                        .rotate_axis_angle_local(&Vec3::UP, entry.rotation[1].to_radians())
                        .rotate_axis_angle_local(&Vec3::EAST, entry.rotation[0].to_radians())
                        .set_position(&Vec3::from(entry.position));

                    objects.push(entity.clone());

                    scene.add_entity(entity);
                }
                (object_name, folder, file_name) => {
                    log::error!(
                        "Cannot load object: {:?} {:?} {:?}",
                        object_name,
                        folder,
                        file_name
                    );
                }
            }
        }

        let module = asset_loader.load_script_module(scene_name)?;

        Ok(Self {
            scene,
            players,
            npcs,
            objects: objects.clone(),
            objects_xz_aabbs: objects
                .iter()
                .map(|e| {
                    // Force the cached `world_transform` to be
                    // recomputed against an identity parent BEFORE
                    // walking the mesh. Without this, every child
                    // entity in the loaded DFF still has its initial
                    // identity world matrix (radiance lazily
                    // propagates transforms via scene ticks, but
                    // `Pal4Scene::load` runs before the first tick),
                    // so `entity_world_xz_aabb` would return an AABB
                    // in mesh-LOCAL space — centred near the origin
                    // — and the player at world (710, _, 864) was
                    // measured at ~1119 units from it, well beyond
                    // any radius. The floor / wall ray casters
                    // already do this same explicit propagation
                    // (see `create_floor_wall_ray_caster`).
                    e.update_world_transform(&Transform::new());
                    entity_world_xz_aabb(e)
                })
                .collect(),
            objects_gob: Some(gob),
            events: events.events,
            module: Some(module),
            triggers,
            uv_anim,
            bsp_entity: Some(bsp_entity),
            floor_entity: floor,
            wall_entity: wall,
            game_context: Some(game_context),
            actor_controller,
        })
    }

    pub fn get_player(&self, player_id: usize) -> ComRc<IEntity> {
        self.players[player_id].clone()
    }

    /// Single party-wide actor controller, or `None` for the empty
    /// scene / when no factory was installed.
    pub fn actor_controller(&self) -> Option<ComRc<IPal4ActorController>> {
        self.actor_controller.clone()
    }

    /// Update the engine-side `Pal4GameContext`'s active leader index.
    /// Script-side actor controllers read this via
    /// `IPal4GameContext::current_leader()` and self-gate per-frame.
    /// No-op on the placeholder scene returned by `new_empty`.
    pub fn set_active_leader(&self, player_id: usize) {
        if let Some(ctx) = &self.game_context {
            ctx.inner::<Pal4GameContext>().set_current_leader(player_id);
        }
    }

    /// Consume the wrapper and return only its inner `ComRc<IScene>`.
    /// Used by the editor's read-only scene preview, which needs the
    /// loaded scene but none of the gameplay-side fields. The dropped
    /// fields (NPCs, GOB objects, events, …) hold entities that the
    /// scene itself already retains via `add_entity`, so they stay
    /// alive for the lifetime of the returned `IScene`.
    pub fn into_inner_scene(self) -> ComRc<IScene> {
        self.scene
    }

    pub fn get_npc(&self, name: &str) -> Option<ComRc<IEntity>> {
        self.npcs.iter().find(|npc| name == npc.name()).cloned()
    }

    pub fn get_object(&self, name: &str) -> Option<ComRc<IEntity>> {
        self.objects
            .iter()
            .find(|object| name == object.name())
            .cloned()
    }

    pub fn get_player_controller(&self, player_id: usize) -> ComRc<IPal4ActorAnimationController> {
        self.players[player_id]
            .get_component(IPal4ActorAnimationController::uuid())
            .unwrap()
            .query_interface::<IPal4ActorAnimationController>()
            .unwrap()
    }

    pub fn get_npc_controller(&self, name: &str) -> Option<ComRc<IPal4ActorAnimationController>> {
        self.get_npc(name)?
            .get_component(IPal4ActorAnimationController::uuid())?
            .query_interface::<IPal4ActorAnimationController>()
    }

    pub fn test_event_triggers(&self) -> Option<&EvfEvent> {
        for trigger in &self.triggers {
            if trigger.triggered.get() {
                return self.events.get(trigger.event_index);
            }
        }

        None
    }

    pub fn test_interaction(
        &self,
        input: Rc<RefCell<dyn InputEngine>>,
        leader: usize,
    ) -> Option<String> {
        let input = input.borrow();
        let down = input.get_key_state(radiance::input::Key::F).pressed()
            || input
                .get_key_state(radiance::input::Key::GamePadEast)
                .pressed();

        if !down {
            return None;
        }

        let position = self.players[leader].world_transform().position();
        let mut min_distance = f32::INFINITY;
        let mut min_function = None;

        for (i, object) in self.objects.iter().enumerate() {
            let entry = &self.objects_gob.as_ref().unwrap().entries[i];
            if entry.research_function == "" {
                continue;
            }

            // Horizontal (XZ) distance from the player to the
            // closest point on the object's bounding rect — or to
            // the entry's anchor when no mesh AABB is available
            // (invisible markers). The Y axis is ignored on
            // purpose: tall objects like ladders/staircases anchor
            // their GOB origin at one end of the climb path while
            // the player stands at the other end. The original
            // engine scopes the interaction prompt by horizontal
            // proximity for exactly this reason; vertical
            // separation is handled by the research handler
            // itself (`func1012`/`func1013` re-snap the player
            // onto the destination point once invoked).
            //
            // Using the *mesh* AABB rather than the anchor matters
            // for the Q01/Q01 ladder #2: its anchor is in the
            // middle of the slope, but the bottom climb-down
            // teleport target ends up ~264 XZ units from the
            // anchor — well outside the entry's 60-unit
            // `trigger_distance` — even though the player is
            // physically next to the visible ladder.
            let xz_dist = match self.objects_xz_aabbs.get(i).and_then(|a| a.as_ref()) {
                Some(&(min_x, max_x, min_z, max_z)) => {
                    let dx = if position.x < min_x {
                        min_x - position.x
                    } else if position.x > max_x {
                        position.x - max_x
                    } else {
                        0.0
                    };
                    let dz = if position.z < min_z {
                        min_z - position.z
                    } else if position.z > max_z {
                        position.z - max_z
                    } else {
                        0.0
                    };
                    (dx * dx + dz * dz).sqrt()
                }
                None => {
                    let opos = object.world_transform().position();
                    let dx = opos.x - position.x;
                    let dz = opos.z - position.z;
                    (dx * dx + dz * dz).sqrt()
                }
            };

            // Use the GOB entry's `trigger_distance` as a slack
            // padding on top of the AABB (defaults to ~60.0 in the
            // shipped data; sound emitters use ~600.0 but they
            // don't have a `research_function` so they're filtered
            // above). Fall back to 50.0 if zero so pathological
            // data doesn't make every object interactive at
            // infinity.
            let radius = if entry.trigger_distance > 0.0 {
                entry.trigger_distance
            } else {
                50.0
            };
            if xz_dist < radius && xz_dist < min_distance {
                min_distance = xz_dist;
                min_function = Some(entry.research_function.to_string().unwrap());
            }
        }

        min_function
    }

    pub fn get_player_metadata(&self, player_id: usize) -> Player {
        match player_id {
            Self::ID_YUN_TIANHE => Player::YunTianhe,
            Self::ID_HAN_LINGSHA => Player::HanLingsha,
            Self::ID_LIU_MENGLI => Player::LiuMengli,
            Self::ID_MURONG_ZIYING => Player::MurongZiying,
            _ => unreachable!(),
        }
    }

    /// Toggle the BSP "world" geometry. No-op on the empty scene
    /// (`Pal4Scene::new_empty`) since `bsp_entity` is `None` there.
    pub fn set_bsp_visible(&self, visible: bool) {
        if let Some(e) = self.bsp_entity.as_ref() {
            e.set_visible(visible);
            e.set_enabled(visible);
        }
    }

    /// Toggle the floor + wall (nav-mesh) collision geometry. These
    /// entities are always added to the scene at load time but start
    /// hidden — flipping them on is a developer aid for inspecting
    /// the walkable surfaces the actor controller raycasts against.
    pub fn set_nav_mesh_visible(&self, visible: bool) {
        if let Some(e) = self.floor_entity.as_ref() {
            e.set_visible(visible);
            e.set_enabled(visible);
        }
        if let Some(e) = self.wall_entity.as_ref() {
            e.set_visible(visible);
            e.set_enabled(visible);
        }
    }
}

fn load_player(asset_loader: &Rc<AssetLoader>, player: Player) -> ComRc<IEntity> {
    let entity = asset_loader
        .load_actor(player.name(), player.actor_name(), Some("C01"))
        .unwrap();

    entity.set_visible(false);
    entity.set_enabled(false);

    entity
}

fn create_floor_wall_ray_caster(
    floor: Option<ComRc<IEntity>>,
    wall: Option<ComRc<IEntity>>,
) -> RayCaster {
    let mut ray_caster = RayCaster::new();
    if let Some(floor) = floor {
        floor.update_world_transform(&Transform::new());
        add_mesh(&mut ray_caster, floor);
    }

    if let Some(wall) = wall {
        wall.update_world_transform(&Transform::new());
        add_mesh(&mut ray_caster, wall);
    }

    ray_caster
}

fn add_mesh(ray_caster: &mut RayCaster, entity: ComRc<IEntity>) {
    for child in entity.children() {
        add_mesh(ray_caster, child);
    }

    let mesh = entity.get_component(IStaticMeshComponent::uuid());
    if let Some(mesh) = mesh {
        let mesh = mesh.query_interface::<IStaticMeshComponent>().unwrap();
        // Bake the entity's *full* world transform (translation +
        // rotation + scale) into every vertex. The previous version
        // only added `entity.world_transform().position()`, which
        // silently dropped any rotation/scale on nested floor/wall
        // sub-entities and produced mis-placed collision triangles
        // (= invisible walls / fall-throughs on affected blocks).
        let world_transform = entity.world_transform();
        let world_matrix = *world_transform.matrix();
        let mesh_inner =
            mesh.inner::<radiance::components::mesh::static_mesh::StaticMeshComponent>();
        let geometries = mesh_inner.get_geometries();
        for geometry in geometries.iter() {
            let v = geometry
                .vertices
                .to_position_vec()
                .into_iter()
                .map(|v| transform_point(&world_matrix, &v))
                .collect();

            let i = geometry.indices.clone();
            ray_caster.add_mesh(v, i);
        }
    }
}

/// Multiply a row-major 4x4 affine matrix by a point `(x, y, z, 1)`
/// and return the resulting 3D point. Used by [`add_mesh`] so the
/// `RayCaster` sees triangles in world space, regardless of which
/// child entity in the floor/wall tree they originated from.
fn transform_point(m: &Mat44, p: &Vec3) -> Vec3 {
    Vec3::new(
        m[0][0] * p.x + m[0][1] * p.y + m[0][2] * p.z + m[0][3],
        m[1][0] * p.x + m[1][1] * p.y + m[1][2] * p.z + m[1][3],
        m[2][0] * p.x + m[2][1] * p.y + m[2][2] * p.z + m[2][3],
    )
}

/// Walk an entity and its children, returning the `(min, max)` world-Y
/// across every vertex of every `IStaticMeshComponent` found, or
/// `None` if the entity tree contains no static meshes. Mirrors the
/// traversal in [`add_mesh`] but accumulates Y bounds instead of
/// feeding a ray caster.
fn entity_world_y_range(entity: &ComRc<IEntity>) -> Option<(f32, f32)> {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    accumulate_y_range(entity, &mut lo, &mut hi);
    if lo.is_finite() && hi.is_finite() && hi >= lo {
        Some((lo, hi))
    } else {
        None
    }
}

fn accumulate_y_range(entity: &ComRc<IEntity>, lo: &mut f32, hi: &mut f32) {
    for child in entity.children() {
        accumulate_y_range(&child, lo, hi);
    }

    if let Some(mesh) = entity.get_component(IStaticMeshComponent::uuid()) {
        let mesh = mesh.query_interface::<IStaticMeshComponent>().unwrap();
        let entity_y = entity.world_transform().position().y;
        let mesh_inner =
            mesh.inner::<radiance::components::mesh::static_mesh::StaticMeshComponent>();
        let geometries = mesh_inner.get_geometries();
        for geometry in geometries.iter() {
            for v in geometry.vertices.to_position_vec() {
                let y = entity_y + v.y;
                if y < *lo {
                    *lo = y;
                }
                if y > *hi {
                    *hi = y;
                }
            }
        }
    }
}

/// Walk an entity tree and return the world-space XZ axis-aligned
/// bounding box `(min_x, max_x, min_z, max_z)` across every vertex of
/// every `IStaticMeshComponent` found, or `None` when the tree has no
/// static mesh. Used by [`Pal4Scene::test_interaction`] so the F-key
/// interaction prompt fires when the player is next to the visible
/// mesh, regardless of where the GOB entry's anchor point sits
/// relative to it (PAL4 ladders anchor in the middle of the slope but
/// the climb handler teleports the player to one of the end points,
/// far from the anchor).
fn entity_world_xz_aabb(entity: &ComRc<IEntity>) -> Option<(f32, f32, f32, f32)> {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    accumulate_xz_aabb(entity, &mut min_x, &mut max_x, &mut min_z, &mut max_z);
    if min_x.is_finite() && max_x.is_finite() && max_x >= min_x && max_z >= min_z {
        Some((min_x, max_x, min_z, max_z))
    } else {
        None
    }
}

fn accumulate_xz_aabb(
    entity: &ComRc<IEntity>,
    min_x: &mut f32,
    max_x: &mut f32,
    min_z: &mut f32,
    max_z: &mut f32,
) {
    for child in entity.children() {
        accumulate_xz_aabb(&child, min_x, max_x, min_z, max_z);
    }

    if let Some(mesh) = entity.get_component(IStaticMeshComponent::uuid()) {
        let mesh = mesh.query_interface::<IStaticMeshComponent>().unwrap();
        // Bake the full world transform (translation + rotation +
        // scale) into each vertex, mirroring `add_mesh` — the
        // entity's `set_position` carries translation but rotation
        // / scale on child sub-frames is otherwise dropped.
        let world_matrix = *entity.world_transform().matrix();
        let mesh_inner =
            mesh.inner::<radiance::components::mesh::static_mesh::StaticMeshComponent>();
        let geometries = mesh_inner.get_geometries();
        for geometry in geometries.iter() {
            for v in geometry.vertices.to_position_vec() {
                let w = transform_point(&world_matrix, &v);
                if w.x < *min_x {
                    *min_x = w.x;
                }
                if w.x > *max_x {
                    *max_x = w.x;
                }
                if w.z < *min_z {
                    *min_z = w.z;
                }
                if w.z > *max_z {
                    *max_z = w.z;
                }
            }
        }
    }
}

/// Walk an entity tree and replace every `Geometry.material` on every
/// `IStaticMeshComponent` with a `GradientYMaterialDef` keyed on
/// `[y_min, y_max]`. Must be called before the owning entity is added
/// to a scene (see `StaticMeshComponent::replace_material`).
fn apply_gradient_material(entity: &ComRc<IEntity>, y_min: f32, y_max: f32) {
    for child in entity.children() {
        apply_gradient_material(&child, y_min, y_max);
    }

    if let Some(mesh) = entity.get_component(IStaticMeshComponent::uuid()) {
        let mesh = mesh.query_interface::<IStaticMeshComponent>().unwrap();
        let mesh_inner =
            mesh.inner::<radiance::components::mesh::static_mesh::StaticMeshComponent>();
        let count = mesh_inner.geometry_count();
        for i in 0..count {
            mesh_inner.replace_material(i, GradientYMaterialDef::create(y_min, y_max));
        }
    }
}

pub struct SceneEventTrigger {
    ray_caster: RayCaster,
    event_index: usize,
    triggered: Cell<bool>,
}

impl SceneEventTrigger {
    pub fn check(&self, origin: &Vec3, direction: &Vec3) {
        // `cast_ray` returns the parametric distance `t` along `direction`
        // (hit point = origin + t * direction). The movement crosses the
        // trigger this frame iff the hit lies within the segment, i.e.
        // `t <= 1.0`. The lower bound (`t > EPSILON`) is enforced by
        // `cast_ray` itself.
        let hit = self.ray_caster.cast_ray(origin, direction);

        self.triggered.set(false);
        if let Some(t) = hit {
            if t <= 1.0 {
                self.triggered.set(true);
            }
        }
    }
}

lazy_static::lazy_static! {
    pub static ref BOX_TRIGGER_INDICES: Vec<u32> = vec![
        0, 2, 1, 0, 3, 2, 0, 4, 7, 0, 7, 3, 0, 5, 4, 0, 1, 5, 6, 1, 2, 6, 5, 1, 6, 2, 3, 6, 3,
        7, 6, 7, 4, 6, 4, 5,
    ];

    pub static ref PLANE_TRIGGER_INDICES: Vec<u32> = vec![0, 1, 2, 2, 1, 3];
}

fn create_trigger_ray_caster(trigger: Vec<Vec3>) -> RayCaster {
    let mut ray_caster = RayCaster::new();
    match trigger.len() {
        4 => {
            ray_caster.add_mesh(trigger, PLANE_TRIGGER_INDICES.clone());
        }
        8 => {
            ray_caster.add_mesh(trigger, BOX_TRIGGER_INDICES.clone());
        }
        _ => panic!("Invalid trigger point count"),
    }

    ray_caster
}
