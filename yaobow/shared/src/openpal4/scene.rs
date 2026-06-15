use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use fileformats::npc::NpcInfoFile;
use fileformats::pal4::{
    evf::EvfEvent,
    gob::{GobCommonProperties, GobFile, GobObjectType},
};
use radiance::{
    audio::Codec,
    comdef::{
        IArmatureComponent, IArmatureComponentExt, IAudioSourceComponent, IComponent, IEntity,
        IEntityExt, IScene, ISceneExt, IStaticMeshComponent,
    },
    components::audio::{AudioNodeConfig, AudioSourceComponent, PlaybackMode, sanitise_interval},
    components::collision::{CollisionWorldComponent, TriggerVolumeComponent},
    input::InputEngine,
    math::{Mat44, Vec3},
    rendering::GradientYMaterialDef,
    scene::{CoreEntity, CoreScene, wrap_scene_camera},
};
use radiance_scripting::services::InputService;

use crate::scripting::angelscript::ScriptModule;

use super::{
    asset_loader::{self, AssetLoader},
    comdef::{
        IPal4ActorAnimationController, IPal4ActorController, IPal4GameContext, IPal4ScriptFactory,
    },
    game_context::Pal4GameContext,
    uv_anim::attach_uv_anim,
};

/// Factory abstraction supplied by the runtime (yaobow) at PAL4 boot:
/// the script app's `IPal4ScriptFactory` COM interface (the
/// yaobow `app.p7` struct conforms to it), which `shared` calls
/// straight through the COM vtable. Mints a single party-wide
/// `IPal4ActorController` covering all four party-member entities +
/// animation handles plus the shared engine surface; the runtime
/// attaches the returned component to a synthetic "party root" entity
/// that parents the four players, so a single controller drives the
/// active leader each frame. Editor previews pass `None` and the scene
/// loads without any per-player controller component attached.
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
    /// Lockstep mapping `objects[i] -> GobFile::entries[index]`.
    /// `objects` only contains *visible* and *marker* entries, while
    /// `objects_gob` retains the full corpus; this Vec lets code
    /// (e.g. `test_interaction`, `gob_movement_metadata`) recover
    /// the GOB entry that authored a loaded entity. Pre-lockstep
    /// indexing — `entries[i]` keyed on the `objects` loop index —
    /// silently consulted the wrong GOB whenever any SOUND / EFFECT
    /// was skipped before a GENERIC interactable in the same block.
    pub(crate) objects_gob_indices: Vec<usize>,
    /// Per-object original `Transform` matrix captured at scene
    /// load, in lockstep with `objects`. Used by `giGOBReset` to
    /// restore the entity to its authored placement after any
    /// number of `giGOBSetPosition` / `giGOBMovment` / `giGOBScale`
    /// calls.
    ///
    /// We snapshot the full `Mat44` (rather than decomposed
    /// `(pos, rot, scale)`) to sidestep two traps: (a) the
    /// load-time transform chain uses `scale_local` and
    /// `rotate_axis_angle_local` which are multiplicative on top of
    /// whatever state the matrix is in, so decomposing and
    /// reapplying them re-orders ops with no guaranteed round-trip;
    /// (b) `Transform::euler` is singular at gimbal lock and can't
    /// round-trip all rotations cleanly.
    pub(crate) objects_initial_transforms: Vec<Mat44>,
    pub(crate) objects_gob: Option<GobFile>,
    pub(crate) events: Vec<EvfEvent>,
    pub(crate) module: Option<Rc<RefCell<ScriptModule>>>,
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

/// Fallback `trigger_distance` for SOUND emitters whose entry has
/// an explicit zero (rare in the corpus, but defending against it
/// at load time avoids a degenerate zero-distance attenuation that
/// would silence the emitter).
const DEFAULT_SOUND_TRIGGER_DISTANCE: f32 = 600.0;

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
            objects_gob_indices: vec![],
            objects_initial_transforms: vec![],
            objects_gob: None,
            events: vec![],
            module: None,
            bsp_entity: None,
            floor_entity: None,
            wall_entity: None,
            game_context: None,
            actor_controller: None,
        }
    }

    /// Synchronous one-shot scene load — pumps a [`Pal4SceneLoader`]
    /// to completion in one call. Preferred entry point for code
    /// paths that don't have a tick budget to spend
    /// (`OpenPAL4Director::load_state` F-key reload, the silent
    /// `giArenaLoad show_loading = 0` flow, editor previews, etc.).
    /// The `Pal4TransitionDirector` instead pumps `step()` one stage
    /// per update tick so the loading overlay's progress bar can
    /// advance between stages.
    pub fn load(
        asset_loader: &Rc<asset_loader::AssetLoader>,
        input: Rc<RefCell<dyn InputEngine>>,
        scene_name: &str,
        block_name: &str,
        actor_controller_factory: Option<&ComRc<IPal4ScriptFactory>>,
    ) -> anyhow::Result<Self> {
        let mut loader = Pal4SceneLoader::new(
            asset_loader.clone(),
            input,
            scene_name.to_string(),
            block_name.to_string(),
            actor_controller_factory.cloned(),
        );
        loop {
            let step = loader.step();
            if let Some(result) = step.done {
                return result;
            }
        }
    }
}

/// Multi-tick staged loader for [`Pal4Scene`]. Each `step()` runs one
/// coarse stage and returns the post-stage progress fraction in
/// `[0, 1]`; on the last stage the constructed [`Pal4Scene`] is
/// returned in `done`. Callers MUST pump `step()` until `done` is
/// `Some(_)` — partial loaders leak the work done so far.
///
/// Stages (end fractions are reported targets, not measured wall-time):
///
/// | # | Work                                          | end frac |
/// |---|-----------------------------------------------|----------|
/// | 0 | `load_scene` (BSP) + scene root               | 0.20     |
/// | 1 | sky / clip / water (+ UV anim) + camera fov   | 0.35     |
/// | 2 | floor / wall meshes, ray caster, add to scene | 0.50     |
/// | 3 | players, events, triggers, actor controller   | 0.70     |
/// | 4 | NPCs                                          | 0.85     |
/// | 5 | GOB objects                                   | 0.95     |
/// | 6 | script module + finalize → `Pal4Scene`        | 1.00     |
pub struct Pal4SceneLoader {
    // Inputs cloned in at construction.
    asset_loader: Rc<asset_loader::AssetLoader>,
    input: Rc<RefCell<dyn InputEngine>>,
    scene_name: String,
    block_name: String,
    actor_controller_factory: Option<ComRc<IPal4ScriptFactory>>,

    // Stage cursor: index of the next stage to run. 0..=6 = pending,
    // 7 = done (the constructed scene was already returned).
    next_stage: u8,

    // Accumulated state. `Option<T>` slots are populated by the
    // stage that produces them and consumed by the finaliser.
    scene: Option<ComRc<IScene>>,
    bsp_entity: Option<ComRc<IEntity>>,
    floor: Option<ComRc<IEntity>>,
    wall: Option<ComRc<IEntity>>,
    players: Option<[ComRc<IEntity>; 4]>,
    events: Vec<EvfEvent>,
    game_context: Option<ComRc<IPal4GameContext>>,
    actor_controller: Option<ComRc<IPal4ActorController>>,
    npcs: Vec<ComRc<IEntity>>,
    objects: Vec<ComRc<IEntity>>,
    objects_gob_indices: Vec<usize>,
    objects_initial_transforms: Vec<Mat44>,
    objects_gob: Option<GobFile>,
    module: Option<Rc<RefCell<ScriptModule>>>,
}

/// One stage's outcome: the cumulative post-stage progress fraction
/// and, on the final stage, the constructed scene (or the first
/// stage error). When `done` is `None` callers should pump `step()`
/// again on a later tick; when `done` is `Some(_)` the loader is
/// exhausted.
pub struct StageProgress {
    pub fraction: f32,
    pub done: Option<anyhow::Result<Pal4Scene>>,
}

impl Pal4SceneLoader {
    pub fn new(
        asset_loader: Rc<asset_loader::AssetLoader>,
        input: Rc<RefCell<dyn InputEngine>>,
        scene_name: String,
        block_name: String,
        actor_controller_factory: Option<ComRc<IPal4ScriptFactory>>,
    ) -> Self {
        Self {
            asset_loader,
            input,
            scene_name,
            block_name,
            actor_controller_factory,
            next_stage: 0,
            scene: None,
            bsp_entity: None,
            floor: None,
            wall: None,
            players: None,
            events: Vec::new(),
            game_context: None,
            actor_controller: None,
            npcs: Vec::new(),
            objects: Vec::new(),
            objects_gob_indices: Vec::new(),
            objects_initial_transforms: Vec::new(),
            objects_gob: None,
            module: None,
        }
    }

    /// Run exactly one stage and return the post-stage progress. On
    /// the final stage the result is returned in `done`. After
    /// `done.is_some()`, further calls return `done: None` with
    /// `fraction: 1.0` to keep the contract honest if a caller pumps
    /// once too many times.
    pub fn step(&mut self) -> StageProgress {
        let stage = self.next_stage;
        if stage >= 7 {
            return StageProgress {
                fraction: 1.0,
                done: None,
            };
        }
        let result: Option<anyhow::Result<()>> = match stage {
            0 => Some(self.stage_bsp()),
            1 => Some(self.stage_sky_clip_water()),
            2 => Some(self.stage_floor_wall()),
            3 => Some(self.stage_players_events_controller()),
            4 => Some(self.stage_npcs()),
            5 => Some(self.stage_gob_objects()),
            6 => {
                // Finalise consumes the loader's state.
                let r = self.stage_finalize();
                self.next_stage = 7;
                return StageProgress {
                    fraction: 1.0,
                    done: Some(r),
                };
            }
            _ => unreachable!(),
        };
        self.next_stage = stage + 1;
        let fraction = match stage {
            0 => 0.20,
            1 => 0.35,
            2 => 0.50,
            3 => 0.70,
            4 => 0.85,
            5 => 0.95,
            _ => 1.0,
        };
        // Surface a stage error early as a `done: Some(Err(_))` so
        // the caller can abort the transition. Stages that
        // tolerate partial failure (NPCs, GOB) handle it inline and
        // return `Ok(())` here.
        if let Some(Err(e)) = result {
            self.next_stage = 7;
            return StageProgress {
                fraction,
                done: Some(Err(e)),
            };
        }
        StageProgress {
            fraction,
            done: None,
        }
    }

    fn stage_bsp(&mut self) -> anyhow::Result<()> {
        let (scene, bsp_entity) = self
            .asset_loader
            .load_scene(&self.scene_name, &self.block_name)?;
        self.scene = Some(scene);
        self.bsp_entity = Some(bsp_entity);
        Ok(())
    }

    fn stage_sky_clip_water(&mut self) -> anyhow::Result<()> {
        let scene = self.scene.as_ref().expect("stage_bsp must run first");

        if !cfg!(vita) {
            let clip = self
                .asset_loader
                .try_load_scene_clip(&self.scene_name, &self.block_name);
            if let Some(clip) = clip {
                scene.add_entity(clip);
            }
        }

        let clip_na = self
            .asset_loader
            .try_load_scene_clip_na(&self.scene_name, &self.block_name);
        if let Some(clip_na) = clip_na {
            scene.add_entity(clip_na);
        }

        let skybox = self
            .asset_loader
            .try_load_scene_sky(&self.scene_name, &self.block_name);
        if let Some(skybox) = skybox {
            scene.add_entity(skybox);
        }

        // Optional water surface (PAL4 scenes that ship a `_water.dff`,
        // e.g. Q01/q01/Q01, Q01/q01/Q01Y). The sibling `_water.uva`
        // drives per-frame UV animation via a self-ticking
        // `UvAnimationComponent` attached to the water entity.
        let water = self
            .asset_loader
            .try_load_scene_water(&self.scene_name, &self.block_name);
        if let Some(water) = water {
            scene.add_entity(water.clone());
            if let Some(dict) = self
                .asset_loader
                .try_load_scene_water_uva(&self.scene_name, &self.block_name)
            {
                log::debug!(
                    "Loaded water UV-anim dict for {}/{}: {} animation(s)",
                    self.scene_name,
                    self.block_name,
                    dict.animations.len()
                );
                attach_uv_anim(&water, &dict);
            }
        }

        scene.camera_mut().set_fov43(45_f32.to_radians());
        Ok(())
    }

    fn stage_floor_wall(&mut self) -> anyhow::Result<()> {
        let scene = self.scene.as_ref().expect("stage_bsp must run first");

        let floor = self
            .asset_loader
            .load_scene_floor(&self.scene_name, &self.block_name);
        let wall = self
            .asset_loader
            .load_scene_wall(&self.scene_name, &self.block_name);
        if floor.is_none() {
            log::warn!(
                "Pal4Scene::load: missing floor mesh for scene='{}' block='{}'. \
                 Floor collision raycast will be empty for this block; the \
                 active leader may freeze in place or fall through geometry.",
                self.scene_name,
                self.block_name
            );
        }
        if wall.is_none() {
            log::warn!(
                "Pal4Scene::load: missing wall mesh for scene='{}' block='{}'. \
                 Wall collision raycast will be empty for this block; the \
                 active leader may walk through walls.",
                self.scene_name,
                self.block_name
            );
        }
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

        // Bake floor + wall into the scene's collision world. The world
        // owns the aggregated caster; the actor controller later pulls
        // a live `IRayCaster` from it via `floor_ray_caster()`.
        let world_com = scene.collision_world();
        let world = world_com.inner::<CollisionWorldComponent>();
        if let Some(f) = floor.as_ref() {
            world.attach_collider(f);
        }
        if let Some(w) = wall.as_ref() {
            world.attach_collider(w);
        }

        // Always add floor + wall so the PAL4 debug overlay can toggle
        // them on at runtime. They default to hidden — matches the old
        // SHOW_FLOOR / SHOW_WALL = false behaviour.
        if let Some(f) = floor.as_ref() {
            f.set_visible(false);
            f.set_enabled(false);
            scene.add_entity(f.clone());
        }
        if let Some(w) = wall.as_ref() {
            w.set_visible(false);
            w.set_enabled(false);
            scene.add_entity(w.clone());
        }

        self.floor = floor;
        self.wall = wall;
        Ok(())
    }

    fn stage_players_events_controller(&mut self) -> anyhow::Result<()> {
        let scene = self.scene.as_ref().expect("stage_bsp must run first");

        let players = [
            load_player(&self.asset_loader, Player::YunTianhe),
            load_player(&self.asset_loader, Player::HanLingsha),
            load_player(&self.asset_loader, Player::LiuMengli),
            load_player(&self.asset_loader, Player::MurongZiying),
        ];

        let events = self
            .asset_loader
            .load_evf(&self.scene_name, &self.block_name)?;

        // Register each EVF event region with the scene collision world
        // as a segment-crossing trigger carrying the event index as its
        // opaque `id` payload. The world owns the holder entity and the
        // registry — the game keeps no parallel Vec.
        let world_com = scene.collision_world();
        let world = world_com.inner::<CollisionWorldComponent>();
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
                        radiance::debug::create_box_entity(self.asset_loader.component_factory());
                    entity.transform().borrow_mut().set_position(point);
                    scene.add_entity(entity);
                }
            }

            if event.vertex_count != 8 && event.vertex_count != 4 {
                continue;
            }

            world.attach_segment_trigger(trigger, i as i64, String::new());
        }

        let game_context = Pal4GameContext::create(scene.clone());

        let actor_controller = if let Some(factory) = self.actor_controller_factory.as_ref() {
            let input_service = InputService::create(self.input.clone());
            let camera_ctrl = wrap_scene_camera(scene.clone());
            let ray_caster_wrapped = scene
                .collision_world()
                .inner::<CollisionWorldComponent>()
                .floor_ray_caster();
            let anims: [ComRc<IPal4ActorAnimationController>; 4] = std::array::from_fn(|i| {
                players[i]
                    .get_component(IPal4ActorAnimationController::uuid())
                    .and_then(|c| c.query_interface::<IPal4ActorAnimationController>())
                    .expect("player must carry an IPal4ActorAnimationController component")
            });
            let [e0, e1, e2, e3] = players.clone();
            let [a0, a1, a2, a3] = anims;
            let controller = factory.make_actor_controller(
                game_context.clone(),
                input_service.clone(),
                e0,
                e1,
                e2,
                e3,
                a0,
                a1,
                a2,
                a3,
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

        self.players = Some(players);
        self.events = events.events;
        self.game_context = Some(game_context);
        self.actor_controller = actor_controller;
        Ok(())
    }

    fn stage_npcs(&mut self) -> anyhow::Result<()> {
        let scene = self.scene.as_ref().expect("stage_bsp must run first");

        // `npcInfo.npc` is optional on disk — some PAL4 blocks don't
        // ship one (e.g. `scenedata/M02/3/` has no `npcInfo.npc`).
        // Treat a missing/unreadable file as "no NPCs" instead of
        // failing the whole scene load, which would `?` out of
        // `giArenaLoad` and abort the surrounding cutscene.
        let npc_info = match self
            .asset_loader
            .load_npc_info(&self.scene_name, &self.block_name)
        {
            Ok(info) => info,
            Err(e) => {
                log::warn!(
                    "Pal4Scene::load: npcInfo.npc missing/unreadable for \
                     scene='{}' block='{}' ({:#}); proceeding with no NPCs",
                    self.scene_name,
                    self.block_name,
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
                    let entity = self.asset_loader.load_actor(
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
        self.npcs = npcs;
        Ok(())
    }

    fn stage_gob_objects(&mut self) -> anyhow::Result<()> {
        let scene = self.scene.as_ref().expect("stage_bsp must run first");

        let mut objects = vec![];
        let mut objects_gob_indices: Vec<usize> = vec![];
        let mut objects_initial_transforms: Vec<Mat44> = vec![];
        let gob = self
            .asset_loader
            .load_gob(&self.scene_name, &self.block_name)?;

        for (i, entry) in gob.entries.iter().enumerate() {
            let object_type = gob.header.object_types[i];
            let object_name = entry.file_name.to_string();
            let folder = entry.folder.to_string();
            let file_name = entry.file_name.to_string();
            let logical_name = entry.name.to_string().ok();

            // SOUND emitters (GOB tag 3): each becomes a dedicated
            // entity carrying an engine-backed `AudioSourceComponent`
            // positioned at the emitter, so OpenAL applies 3D
            // attenuation/panning relative to the camera listener.
            // Built independently of the visible-entity skip below so
            // the node survives the rendering-path `continue`.
            if object_type == GobObjectType::SOUND {
                if let (Some(name), Some(min_t), Some(max_t)) = (
                    entry.sound_name(),
                    entry.sound_min_time(),
                    entry.sound_max_time(),
                ) {
                    // A `0/0` interval marks a seamless looping ambient
                    // bed (river/waterfall); positive intervals are
                    // intermittent random one-shots.
                    let mode = if min_t == 0.0 && max_t == 0.0 {
                        PlaybackMode::Loop
                    } else {
                        let (min, max) = sanitise_interval(min_t, max_t);
                        PlaybackMode::RandomInterval { min, max }
                    };
                    let trigger_distance = if entry.trigger_distance > 0.0 {
                        entry.trigger_distance
                    } else {
                        DEFAULT_SOUND_TRIGGER_DISTANCE
                    };
                    // Map the GOB trigger distance onto the engine's
                    // clamped-linear attenuation: full volume within a
                    // quarter of it, fading to silence at the trigger
                    // distance (replacing the old binary in/out gate).
                    let config = AudioNodeConfig {
                        spatial: true,
                        mode,
                        gain: 1.0,
                        reference_distance: trigger_distance * 0.25,
                        rolloff_factor: 1.0,
                        max_distance: trigger_distance,
                    };

                    match self.asset_loader.load_sound(&name, "wav") {
                        Ok(data) => {
                            let entity = CoreEntity::create(format!("sound:{}", name), false);
                            entity
                                .transform()
                                .borrow_mut()
                                .set_position(&Vec3::from(entry.position));
                            let component = AudioSourceComponent::create(
                                entity.clone(),
                                self.asset_loader.audio_engine(),
                                data,
                                Codec::Wav,
                                config,
                            );
                            entity.add_component(
                                IAudioSourceComponent::uuid(),
                                component.query_interface::<IComponent>().unwrap(),
                            );
                            scene.add_entity(entity);
                        }
                        Err(e) => {
                            log::warn!("Cannot load ambient sound '{}': {:#}", name, e)
                        }
                    }
                }
            }

            match (object_name, folder, file_name) {
                (Ok(object_name), Ok(folder), Ok(file_name)) => {
                    let entity_name = logical_name.unwrap_or_else(|| object_name.clone());

                    if matches!(object_type, GobObjectType::EFFECT | GobObjectType::SOUND) {
                        continue;
                    }

                    let entity = if object_type == GobObjectType::MARKER {
                        CoreEntity::create(entity_name.clone(), true)
                    } else {
                        self.asset_loader
                            .load_object(&entity_name, &folder, &file_name)
                            .unwrap_or_else(|| {
                                log::error!(
                                    "Cannot load object: {:?} {:?} {:?}",
                                    entity_name,
                                    folder,
                                    file_name
                                );
                                CoreEntity::create(entity_name.clone(), false)
                            })
                    };

                    // Cutscene-only set-dressing (GENERIC type with the
                    // "scripted" bit clear) must start hidden; the plot script
                    // reveals it via `giSetObjectVisible` (-> `enable_object`,
                    // which re-enables it). Disable it too so an invisible prop
                    // doesn't leave behind collision / examine prompts.
                    let hidden_cutscene_prop = entry.is_hidden_cutscene_prop(object_type);
                    let initially_hidden = object_type == GobObjectType::MARKER
                        || entry.is_initially_hidden()
                        || hidden_cutscene_prop;
                    entity.set_visible(!initially_hidden);
                    entity.set_enabled(!hidden_cutscene_prop);

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

                    let initial_matrix = *entity.transform().borrow().matrix();

                    // ACTION props (tag 6) auto-play their own animation
                    // (`<folder>/<file_name>.anm`) when the block loads:
                    // doors swing open, banners wave, flowers sway. The
                    // object DFF already built a self-ticking armature via
                    // the shared HAnim path, so we just install + start
                    // the clip here.
                    //
                    // Looping is governed by `play-times`: a negative
                    // count (the shipped corpus is almost all `-1`) means
                    // "repeat indefinitely". `holding-end` only matters
                    // for a *finite* count — freeze on the final keyframe
                    // after the last repeat instead of snapping back.
                    if object_type == GobObjectType::ACTION && entry.action_default_play() {
                        let play_times = entry.action_play_times().unwrap_or(-1);
                        let looping = play_times < 0;
                        let hold_end = !looping && entry.action_holding_end();
                        play_object_default_animation(
                            &entity,
                            &self.asset_loader,
                            &folder,
                            &file_name,
                            looping,
                            hold_end,
                        );
                    }

                    objects.push(entity.clone());
                    objects_gob_indices.push(i);
                    objects_initial_transforms.push(initial_matrix);

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

        // Surface duplicate-name collisions in the loaded `objects` set
        // (kept at end of stage so a future re-ordering of GOB
        // processing doesn't lose the warning).
        {
            use std::collections::HashSet;
            let mut seen = HashSet::new();
            for entity in &objects {
                let name = entity.name();
                if !seen.insert(name.clone()) {
                    log::warn!(
                        "Pal4Scene::load: duplicate object name '{}' in scene='{}' block='{}'; \
                         `get_object` will return the first occurrence",
                        name,
                        self.scene_name,
                        self.block_name
                    );
                }
            }
        }

        self.objects = objects;
        self.objects_gob_indices = objects_gob_indices;
        self.objects_initial_transforms = objects_initial_transforms;
        self.objects_gob = Some(gob);
        Ok(())
    }

    fn stage_finalize(&mut self) -> anyhow::Result<Pal4Scene> {
        let module = self.asset_loader.load_script_module(&self.scene_name)?;
        self.module = Some(module);

        let scene = self.scene.take().expect("stage_bsp must run first");
        let bsp_entity = self.bsp_entity.take();
        let players = self
            .players
            .take()
            .expect("stage_players_events_controller must run first");
        let game_context = self
            .game_context
            .take()
            .expect("stage_players_events_controller must run first");
        let module = self.module.take().expect("just populated above");
        let objects = std::mem::take(&mut self.objects);
        let objects_gob_indices = std::mem::take(&mut self.objects_gob_indices);
        let objects_gob = self.objects_gob.take();

        // Register a proximity interaction trigger with the scene
        // collision world for each interactable GOB (non-empty
        // `research_function`). The world derives the volume's XZ
        // rectangle from the object's mesh extent, attaches the
        // component to the entity, and owns the registry — the game
        // keeps no parallel Vec. Payload: `id` = GOB index,
        // `tag` = research-function, `radius` = `trigger_distance`.
        {
            let world_com = scene.collision_world();
            let world = world_com.inner::<CollisionWorldComponent>();
            for (i, e) in objects.iter().enumerate() {
                let Some(&gob_index) = objects_gob_indices.get(i) else {
                    continue;
                };
                let Some(entry) = objects_gob.as_ref().and_then(|g| g.entries.get(gob_index)) else {
                    continue;
                };
                let research = entry.research_function.to_string().unwrap_or_default();
                if research.is_empty() {
                    continue;
                }
                let radius = if entry.trigger_distance > 0.0 {
                    entry.trigger_distance
                } else {
                    50.0
                };
                world.attach_proximity_trigger(e, gob_index as i64, research, radius);
            }
        }

        Ok(Pal4Scene {
            scene,
            players,
            npcs: std::mem::take(&mut self.npcs),
            objects,
            objects_gob_indices,
            objects_initial_transforms: std::mem::take(&mut self.objects_initial_transforms),
            objects_gob,
            events: std::mem::take(&mut self.events),
            module: Some(module),
            bsp_entity,
            floor_entity: self.floor.take(),
            wall_entity: self.wall.take(),
            game_context: Some(game_context),
            actor_controller: self.actor_controller.take(),
        })
    }
}

impl Pal4Scene {
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
        let event_index = self
            .scene
            .collision_world()
            .inner::<CollisionWorldComponent>()
            .fired_segment_trigger()? as usize;
        self.events.get(event_index)
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

        // The scene collision world holds a proximity trigger per
        // interactable GOB (payload `tag` = research-function,
        // `radius` = `trigger_distance`); it measures horizontal (XZ)
        // distance, ignoring Y on purpose — tall props like the Q01/Q01
        // ladders anchor at one end of the climb path while the player
        // stands at the other; the research handler re-snaps the player
        // onto the destination once invoked. Ask for the nearest volume
        // within its own radius and read back its research-function tag.
        let position = self.players[leader].world_transform().position();
        self.scene
            .collision_world()
            .inner::<CollisionWorldComponent>()
            .nearest_proximity_trigger(&position)
            .map(|volume| volume.inner::<TriggerVolumeComponent>().tag())
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

    /// Index lookup for `objects` by logical name. Returns the
    /// position of the first match (mirrors `get_object`'s
    /// first-match-wins semantics) so callers can index into the
    /// parallel `object_triggers` / `objects_initial_transforms` Vecs.
    fn object_index_by_name(&self, name: &str) -> Option<usize> {
        self.objects.iter().position(|o| o.name() == name)
    }

    /// Resolve the raw GOB `folder` (e.g. `gamedata\PALObject\OM01\`)
    /// that authored the loaded object `name`, so callers can locate
    /// its sibling `.anm`/`.dff`. Returns `None` for unknown objects or
    /// when the GOB corpus wasn't retained.
    pub fn get_object_folder(&self, name: &str) -> Option<String> {
        let idx = self.object_index_by_name(name)?;
        let gob = self.objects_gob.as_ref()?;
        let gob_idx = *self.objects_gob_indices.get(idx)?;
        gob.entries.get(gob_idx)?.folder.to_string().ok()
    }

    /// Set an object's local position to `(x, y, z)`. The interactable
    /// object's proximity trigger volume tracks the entity translation
    /// automatically, so no cache invalidation is needed here. Returns
    /// `true` on hit, `false` on miss; warns are the
    /// caller's responsibility (so the script-side stub can attach
    /// the function name).
    pub fn set_object_position(&mut self, name: &str, x: f32, y: f32, z: f32) -> bool {
        let Some(idx) = self.object_index_by_name(name) else {
            return false;
        };
        self.objects[idx]
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(x, y, z));
        true
    }

    /// Snap an object's position AND its Y-axis (yaw) rotation. The
    /// yaw is set absolutely: the matrix is rebuilt from the
    /// snapshotted initial transform's rotation, with the new yaw
    /// composed onto an identity rotation. This avoids the
    /// accumulation trap where repeated `rotate_axis_angle_local`
    /// calls compound rather than overwrite.
    ///
    /// `rot_deg` rotates around `Vec3::UP` (y-axis); this matches
    /// how PAL4 props are placed.
    pub fn set_object_position_and_yaw(
        &mut self,
        name: &str,
        x: f32,
        y: f32,
        z: f32,
        rot_deg: f32,
    ) -> bool {
        let Some(idx) = self.object_index_by_name(name) else {
            return false;
        };
        let xform = self.objects[idx].transform();
        let mut t = xform.borrow_mut();
        t.clear_rotation();
        t.rotate_axis_angle_local(&Vec3::UP, rot_deg.to_radians());
        t.set_position(&Vec3::new(x, y, z));
        drop(t);
        true
    }

    /// Set an object's local scale to `(x_scale, y_scale, x_scale)`
    /// (Z reuses `x_scale` because the script API exposes only two
    /// axes and PAL4 props are y-up). Rotation is preserved as
    /// identity afterwards because we rebuild the matrix from
    /// scratch — the only correct way to make this an *absolute*
    /// scale rather than multiplying onto an already-scaled state.
    /// Position is preserved.
    pub fn set_object_scale_xy(&mut self, name: &str, x_scale: f32, y_scale: f32) -> bool {
        let Some(idx) = self.object_index_by_name(name) else {
            return false;
        };
        let xform = self.objects[idx].transform();
        let mut t = xform.borrow_mut();
        let pos = t.position();
        t.set_matrix(Mat44::new_identity());
        t.scale_local(&Vec3::new(x_scale, y_scale, x_scale));
        t.set_position(&pos);
        drop(t);
        true
    }

    /// Restore an object's transform to its load-time snapshot in
    /// `objects_initial_transforms`. The interactable's proximity
    /// trigger volume tracks the entity translation automatically, so
    /// no cache invalidation is needed.
    pub fn reset_object(&mut self, name: &str) -> bool {
        let Some(idx) = self.object_index_by_name(name) else {
            return false;
        };
        let saved = self.objects_initial_transforms[idx];
        self.objects[idx].transform().borrow_mut().set_matrix(saved);
        true
    }
}

/// Look up an object entity's `IArmatureComponent`, if it has one.
/// Game-object DFFs only carry an armature when they ship an HAnim
/// skeleton (i.e. an animated prop); static props return `None`.
pub(crate) fn object_armature(entity: &ComRc<IEntity>) -> Option<ComRc<IArmatureComponent>> {
    entity
        .get_component(IArmatureComponent::uuid())
        .and_then(|c| c.query_interface::<IArmatureComponent>())
}

/// Start an animation clip on a game object's armature.
///
/// `looping` continuously restarts the clip; otherwise `hold_end`
/// decides whether the prop freezes on its final keyframe (doors that
/// stay open) or stops and resets to the start. No-op (with a warning)
/// when the entity has no armature.
pub(crate) fn play_object_animation(
    entity: &ComRc<IEntity>,
    keyframes: Vec<Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>>,
    events: Vec<radiance::components::mesh::event::AnimationEvent>,
    looping: bool,
    hold_end: bool,
) -> bool {
    let Some(armature) = object_armature(entity) else {
        log::warn!(
            "play_object_animation: object '{}' has no armature; cannot animate",
            entity.name()
        );
        return false;
    };

    armature.set_animation(keyframes, events);
    armature.set_looping(looping);
    armature.set_hold_end(hold_end);
    armature.play();
    true
}

/// Load and start an ACTION prop's default animation
/// (`<folder>/<file_name>.anm`). `looping` repeats the clip
/// indefinitely (the `play-times < 0` case); when not looping,
/// `hold_end` freezes on the final keyframe instead of resetting.
fn play_object_default_animation(
    entity: &ComRc<IEntity>,
    asset_loader: &Rc<AssetLoader>,
    folder: &str,
    file_name: &str,
    looping: bool,
    hold_end: bool,
) {
    if object_armature(entity).is_none() {
        // Some ACTION entries (e.g. DHA) carry the action params but
        // ship no skeleton / `.anm`; nothing to animate.
        return;
    }

    match asset_loader.load_object_animation(folder, file_name) {
        Ok(anim) => {
            play_object_animation(entity, anim.keyframes, anim.events, looping, hold_end);
        }
        Err(e) => {
            log::warn!(
                "play_object_default_animation: no animation for object '{}' ({}{}.anm): {:#}",
                entity.name(),
                folder,
                file_name,
                e
            );
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

#[cfg(test)]
mod tests {
    use super::*;
    use radiance::components::collision::TriggerShape;
    use radiance::math::Transform;

    /// Reset after a set-position call must restore the snapshot
    /// matrix exactly. Locks the `Mat44` round-trip used by
    /// `giGOBReset` so a future change to scale/clear_rotation
    /// ordering does not silently lose the load-time placement.
    #[test]
    fn reset_object_restores_initial_transform() {
        let mut s = Pal4Scene::new_empty();
        let entity = CoreEntity::create("marker001".to_string(), true);
        entity
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(10.0, 20.0, 30.0));
        let snap = *entity.transform().borrow().matrix();
        s.objects.push(entity);
        s.objects_gob_indices.push(0);
        s.objects_initial_transforms.push(snap);

        // Mutate then reset.
        assert!(s.set_object_position("marker001", 1.0, 2.0, 3.0));
        let p = s.objects[0].transform().borrow().position();
        assert!((p.x - 1.0).abs() < 1e-4 && (p.y - 2.0).abs() < 1e-4 && (p.z - 3.0).abs() < 1e-4);
        assert!(s.reset_object("marker001"));
        let p = s.objects[0].transform().borrow().position();
        assert!(
            (p.x - 10.0).abs() < 1e-4 && (p.y - 20.0).abs() < 1e-4 && (p.z - 30.0).abs() < 1e-4,
            "reset must restore position, got {:?}",
            p,
        );
    }

    /// Position-and-yaw mutation must NOT accumulate yaw across
    /// repeated calls — a script issuing `giGOBMovment(..., rot=90)`
    /// three times in a row must end at 90°, not 270°. This is the
    /// invariant `clear_rotation()` exists to enforce; we verify it
    /// behaviourally rather than reading euler back (which is
    /// singular at gimbal lock).
    #[test]
    fn set_object_position_and_yaw_does_not_accumulate() {
        let mut s = Pal4Scene::new_empty();
        let entity = CoreEntity::create("door001".to_string(), true);
        let snap = *entity.transform().borrow().matrix();
        s.objects.push(entity);
        s.objects_gob_indices.push(0);
        s.objects_initial_transforms.push(snap);

        // Call three times with the same yaw; the resulting
        // rotation matrix must equal a single yaw=90 application.
        for _ in 0..3 {
            assert!(s.set_object_position_and_yaw("door001", 0.0, 0.0, 0.0, 90.0));
        }
        let m_repeat = *s.objects[0].transform().borrow().matrix();

        // Reset and apply once.
        s.reset_object("door001");
        assert!(s.set_object_position_and_yaw("door001", 0.0, 0.0, 0.0, 90.0));
        let m_once = *s.objects[0].transform().borrow().matrix();

        // Rotational submatrices must match.
        for r in 0..3 {
            for c in 0..3 {
                assert!(
                    (m_repeat[r][c] - m_once[r][c]).abs() < 1e-4,
                    "yaw accumulated: m_repeat[{r}][{c}]={} vs m_once[{r}][{c}]={}",
                    m_repeat[r][c],
                    m_once[r][c],
                );
            }
        }
    }

    /// A proximity `TriggerVolumeComponent` seeded from an object's
    /// load-time anchor must track the entity's translation, so the
    /// interaction probe stays correct after a `giGOB*` move without
    /// any manual cache invalidation. This is the behaviour that
    /// replaced the old `objects_xz_aabbs` cache + invalidation hooks.
    #[test]
    fn proximity_volume_tracks_entity_translation() {
        let entity = CoreEntity::create("crate001".to_string(), true);
        entity
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(0.0, 0.0, 0.0));
        entity.update_world_transform(&Transform::new());

        let volume = TriggerVolumeComponent::create(
            entity.clone(),
            TriggerShape::ProximityPoint {
                point: Vec3::new(0.0, 0.0, 0.0),
                radius: 10.0,
            },
            0,
            "func1012".to_string(),
        )
        .unwrap();
        let inner = volume.inner::<TriggerVolumeComponent>();

        // Player at the origin is on top of the anchor.
        assert!(inner.distance_xz(&Vec3::new(0.0, 0.0, 0.0)) < 1e-4);

        // Move the object +100 in X; the volume must follow, so a probe
        // at x=100 is again at distance ~0 and one at x=0 is ~100 away.
        entity
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(100.0, 0.0, 0.0));
        entity.update_world_transform(&Transform::new());
        assert!(inner.distance_xz(&Vec3::new(100.0, 0.0, 0.0)) < 1e-4);
        assert!((inner.distance_xz(&Vec3::new(0.0, 0.0, 0.0)) - 100.0).abs() < 1e-3);
    }

    /// Mutators must return `false` (and log) for unknown names
    /// rather than panicking — PAL4 scripts cross-reference names
    /// across blocks all the time.
    #[test]
    fn mutators_return_false_on_unknown_name() {
        let mut s = Pal4Scene::new_empty();
        assert!(!s.set_object_position("nope", 0.0, 0.0, 0.0));
        assert!(!s.set_object_position_and_yaw("nope", 0.0, 0.0, 0.0, 0.0));
        assert!(!s.set_object_scale_xy("nope", 1.0, 1.0));
        assert!(!s.reset_object("nope"));
    }
}
