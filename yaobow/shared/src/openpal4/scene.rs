use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use fileformats::npc::NpcInfoFile;
use fileformats::pal4::{
    evf::EvfEvent,
    gob::{GobCommonProperties, GobObjectType},
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
        IPal4ActorAnimationController, IPal4ActorController, IPal4GameContext,
        IPal4ObjectComponent, IPal4ScriptFactory,
    },
    game_context::Pal4GameContext,
    object_component::Pal4ObjectComponent,
    uv_anim::attach_uv_anim,
};

/// Engine entity tags for PAL4 scene roles. Entities are tagged at
/// load time so `Pal4Scene` can rediscover them through the generic
/// `ISceneExt` tag queries instead of keeping a parallel handle `Vec`.
pub(crate) const TAG_NPC: &str = "pal4_npc";
pub(crate) const TAG_OBJECT: &str = "pal4_object";
pub(crate) const TAG_FLOOR: &str = "pal4_floor";
pub(crate) const TAG_WALL: &str = "pal4_wall";
pub(crate) const TAG_BSP: &str = "pal4_bsp";

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
    pub(crate) events: Vec<EvfEvent>,
    pub(crate) module: Option<Rc<RefCell<ScriptModule>>>,
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
            events: vec![],
            module: None,
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
    players: Option<[ComRc<IEntity>; 4]>,
    events: Vec<EvfEvent>,
    game_context: Option<ComRc<IPal4GameContext>>,
    actor_controller: Option<ComRc<IPal4ActorController>>,
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
            players: None,
            events: Vec::new(),
            game_context: None,
            actor_controller: None,
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
        bsp_entity.add_tag(TAG_BSP);
        self.scene = Some(scene);
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
        // SHOW_FLOOR / SHOW_WALL = false behaviour. They are tagged so
        // `set_nav_mesh_visible` can rediscover them via the scene tag
        // query rather than a stored handle.
        if let Some(f) = floor.as_ref() {
            f.set_visible(false);
            f.set_enabled(false);
            f.add_tag(TAG_FLOOR);
            scene.add_entity(f.clone());
        }
        if let Some(w) = wall.as_ref() {
            w.set_visible(false);
            w.set_enabled(false);
            w.add_tag(TAG_WALL);
            scene.add_entity(w.clone());
        }

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
                        entity.add_tag(TAG_NPC);
                        entity
                            .transform()
                            .borrow_mut()
                            .set_position(&Vec3::new_zeros())
                            .rotate_axis_angle_local(&Vec3::BACK, npc.rotation[2].to_radians())
                            .rotate_axis_angle_local(&Vec3::UP, npc.rotation[1].to_radians())
                            .rotate_axis_angle_local(&Vec3::EAST, npc.rotation[0].to_radians())
                            .set_position(&Vec3::from(npc.position));

                        scene.add_entity(entity);
                    }
                }
                Err(e) => {
                    log::error!("Cannot load actor: {}", e)
                }
            }
        }
        Ok(())
    }

    fn stage_gob_objects(&mut self) -> anyhow::Result<()> {
        let scene = self.scene.as_ref().expect("stage_bsp must run first");

        let mut objects = vec![];
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

                    // Tag the object for engine-side discovery and ride
                    // its authoring `GobEntry` + object-type + load-time
                    // transform on the entity via `Pal4ObjectComponent`
                    // (replacing the old index-parallel
                    // `objects_gob_indices` / `objects_initial_transforms`
                    // Vecs and the `objects_gob` lookup table).
                    entity.add_tag(TAG_OBJECT);
                    entity.add_component(
                        IPal4ObjectComponent::uuid(),
                        Pal4ObjectComponent::create(entry.clone(), object_type, initial_matrix)
                            .query_interface::<IComponent>()
                            .unwrap(),
                    );

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

        Ok(())
    }

    fn stage_finalize(&mut self) -> anyhow::Result<Pal4Scene> {
        let module = self.asset_loader.load_script_module(&self.scene_name)?;
        self.module = Some(module);

        let scene = self.scene.take().expect("stage_bsp must run first");
        let players = self
            .players
            .take()
            .expect("stage_players_events_controller must run first");
        let game_context = self
            .game_context
            .take()
            .expect("stage_players_events_controller must run first");
        let module = self.module.take().expect("just populated above");

        // Register a proximity interaction trigger with the scene
        // collision world for each interactable GOB (non-empty
        // `research_function`). The world derives the volume's XZ
        // rectangle from the object's mesh extent, attaches the
        // component to the entity, and owns the registry — the game
        // keeps no parallel Vec. Payload: `tag` = research-function,
        // `radius` = `trigger_distance`. Object entities are found via
        // the `TAG_OBJECT` tag query and their authoring `GobEntry` is
        // read back from each `Pal4ObjectComponent`.
        {
            let world_com = scene.collision_world();
            let world = world_com.inner::<CollisionWorldComponent>();
            for (i, e) in scene
                .find_entities_by_tag(TAG_OBJECT)
                .into_iter()
                .enumerate()
            {
                let Some(component) = object_component(&e) else {
                    continue;
                };
                let component = component.inner::<Pal4ObjectComponent>();
                let research = component.research_function();
                if research.is_empty() {
                    continue;
                }
                let trigger_distance = component.trigger_distance();
                let radius = if trigger_distance > 0.0 {
                    trigger_distance
                } else {
                    50.0
                };
                // The trigger `id` payload is never read back (callers
                // consume only the `tag`), so a per-loop counter suffices.
                world.attach_proximity_trigger(&e, i as i64, research, radius);
            }
        }

        Ok(Pal4Scene {
            scene,
            players,
            events: std::mem::take(&mut self.events),
            module: Some(module),
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
    /// loaded scene but none of the gameplay-side fields. All loaded
    /// entities (NPCs, GOB objects, floor/wall/BSP, …) are retained by
    /// the scene itself via `add_entity` — and are discoverable through
    /// their role tags — so they stay alive for the lifetime of the
    /// returned `IScene`.
    pub fn into_inner_scene(self) -> ComRc<IScene> {
        self.scene
    }

    pub fn get_npc(&self, name: &str) -> Option<ComRc<IEntity>> {
        self.scene.find_entity_by_tag_and_name(TAG_NPC, name)
    }

    /// All NPC entities in the scene (tagged `TAG_NPC`). Used by the
    /// agent server to snapshot NPCs with their live positions.
    pub fn npcs(&self) -> Vec<ComRc<IEntity>> {
        self.scene.find_entities_by_tag(TAG_NPC)
    }

    pub fn get_object(&self, name: &str) -> Option<ComRc<IEntity>> {
        self.scene.find_entity_by_tag_and_name(TAG_OBJECT, name)
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
    /// (`Pal4Scene::new_empty`) which has no `TAG_BSP` entity.
    pub fn set_bsp_visible(&self, visible: bool) {
        if let Some(e) = self.scene.find_entity_by_tag(TAG_BSP) {
            e.set_visible(visible);
            e.set_enabled(visible);
        }
    }

    /// Toggle the floor + wall (nav-mesh) collision geometry. These
    /// entities are always added to the scene at load time but start
    /// hidden — flipping them on is a developer aid for inspecting
    /// the walkable surfaces the actor controller raycasts against.
    pub fn set_nav_mesh_visible(&self, visible: bool) {
        for e in self
            .scene
            .find_entities_by_tag(TAG_FLOOR)
            .into_iter()
            .chain(self.scene.find_entities_by_tag(TAG_WALL))
        {
            e.set_visible(visible);
            e.set_enabled(visible);
        }
    }

    /// Resolve the raw GOB `folder` (e.g. `gamedata\PALObject\OM01\`)
    /// that authored the loaded object `name`, so callers can locate
    /// its sibling `.anm`/`.dff`. Returns `None` for unknown objects.
    pub fn get_object_folder(&self, name: &str) -> Option<String> {
        let entity = self.get_object(name)?;
        Some(
            object_component(&entity)?
                .inner::<Pal4ObjectComponent>()
                .folder(),
        )
    }

    /// All loaded GOB object entities (those tagged `TAG_OBJECT`, each
    /// carrying a [`Pal4ObjectComponent`]). Entity-less GOB entries
    /// (EFFECT placeholders, SOUND emitters) are *not* represented —
    /// the parsed `GobFile` is no longer retained after load.
    pub fn object_entities(&self) -> Vec<ComRc<IEntity>> {
        self.scene.find_entities_by_tag(TAG_OBJECT)
    }

    /// Set an object's local position to `(x, y, z)`. The interactable
    /// object's proximity trigger volume tracks the entity translation
    /// automatically, so no cache invalidation is needed here. Returns
    /// `true` on hit, `false` on miss; warns are the
    /// caller's responsibility (so the script-side stub can attach
    /// the function name).
    pub fn set_object_position(&mut self, name: &str, x: f32, y: f32, z: f32) -> bool {
        let Some(entity) = self.get_object(name) else {
            return false;
        };
        entity
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
        let Some(entity) = self.get_object(name) else {
            return false;
        };
        let xform = entity.transform();
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
        let Some(entity) = self.get_object(name) else {
            return false;
        };
        let xform = entity.transform();
        let mut t = xform.borrow_mut();
        let pos = t.position();
        t.set_matrix(Mat44::new_identity());
        t.scale_local(&Vec3::new(x_scale, y_scale, x_scale));
        t.set_position(&pos);
        drop(t);
        true
    }

    /// Restore an object's transform to its load-time snapshot held on
    /// its [`Pal4ObjectComponent`]. The interactable's proximity
    /// trigger volume tracks the entity translation automatically, so
    /// no cache invalidation is needed.
    pub fn reset_object(&mut self, name: &str) -> bool {
        let Some(entity) = self.get_object(name) else {
            return false;
        };
        let Some(saved) =
            object_component(&entity).map(|c| c.inner::<Pal4ObjectComponent>().initial_transform())
        else {
            return false;
        };
        entity.transform().borrow_mut().set_matrix(saved);
        true
    }
}

/// Look up an object entity's [`Pal4ObjectComponent`] (its GOB index +
/// load-time transform), if present. Object entities carry one; markers
/// and other entities do not.
pub(crate) fn object_component(entity: &ComRc<IEntity>) -> Option<ComRc<IPal4ObjectComponent>> {
    entity
        .get_component(IPal4ObjectComponent::uuid())
        .and_then(|c| c.query_interface::<IPal4ObjectComponent>())
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

    /// A minimal placeholder `GobEntry` for tests that only exercise
    /// the component's transform plumbing (not its GOB metadata).
    fn test_gob_entry() -> fileformats::pal4::gob::GobEntry {
        use fileformats::pal4::gob::{GobEntry, GobPropertyI32};
        let i32_prop = || GobPropertyI32 {
            ty: 1,
            name: String::new(),
            value: 0,
        };
        GobEntry {
            name: "".into(),
            folder: "".into(),
            file_name: "".into(),
            file_name2: "".into(),
            position: [0.0; 3],
            rotation: [0.0; 3],
            research_function: "".into(),
            flags: [0; 3],
            trigger_distance: 0.0,
            active: 1,
            game_object: i32_prop(),
            properties: vec![],
            parameters_begin: i32_prop(),
            parameters: vec![],
        }
    }

    /// Tag `entity` as a GOB object, give it a `Pal4ObjectComponent`
    /// carrying its snapshot transform, and add it to the scene so the
    /// `TAG_OBJECT` tag query can find it — the test-side equivalent of
    /// what `stage_gob_objects` does at load.
    fn add_test_object(s: &Pal4Scene, entity: ComRc<IEntity>, snap: Mat44) {
        entity.add_tag(TAG_OBJECT);
        entity.add_component(
            IPal4ObjectComponent::uuid(),
            Pal4ObjectComponent::create(test_gob_entry(), GobObjectType::MARKER, snap)
                .query_interface::<IComponent>()
                .unwrap(),
        );
        s.scene.add_entity(entity);
    }

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
        add_test_object(&s, entity, snap);

        // Mutate then reset.
        assert!(s.set_object_position("marker001", 1.0, 2.0, 3.0));
        let p = s
            .get_object("marker001")
            .unwrap()
            .transform()
            .borrow()
            .position();
        assert!((p.x - 1.0).abs() < 1e-4 && (p.y - 2.0).abs() < 1e-4 && (p.z - 3.0).abs() < 1e-4);
        assert!(s.reset_object("marker001"));
        let p = s
            .get_object("marker001")
            .unwrap()
            .transform()
            .borrow()
            .position();
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
        add_test_object(&s, entity, snap);

        // Call three times with the same yaw; the resulting
        // rotation matrix must equal a single yaw=90 application.
        for _ in 0..3 {
            assert!(s.set_object_position_and_yaw("door001", 0.0, 0.0, 0.0, 90.0));
        }
        let m_repeat = *s
            .get_object("door001")
            .unwrap()
            .transform()
            .borrow()
            .matrix();

        // Reset and apply once.
        s.reset_object("door001");
        assert!(s.set_object_position_and_yaw("door001", 0.0, 0.0, 0.0, 90.0));
        let m_once = *s
            .get_object("door001")
            .unwrap()
            .transform()
            .borrow()
            .matrix();

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
