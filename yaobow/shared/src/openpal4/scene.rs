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
    comdef::{IEntity, IEntityExt, IScene, ISceneExt, IStaticMeshComponent},
    input::InputEngine,
    math::{Mat44, Transform, Vec3},
    rendering::GradientYMaterialDef,
    scene::{CoreEntity, CoreScene, wrap_scene_camera},
    utils::ray_casting::{RayCaster, wrap_ray_caster},
};
use radiance_scripting::services::InputService;

use crate::scripting::angelscript::ScriptModule;

use super::{
    asset_loader::{self, AssetLoader},
    comdef::{
        IPal4ActorAnimationController, IPal4ActorController, IPal4ScriptFactory,
        IPal4GameContext,
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
    ///
    /// Slots are set back to `None` whenever a `giGOB*` script
    /// function mutates the corresponding object's transform —
    /// `test_interaction` then falls back to anchor-based distance
    /// for that entry, which is correct (if slightly less precise)
    /// after the move.
    pub(crate) objects_xz_aabbs: Vec<Option<(f32, f32, f32, f32)>>,
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
    /// Per-scene ambient sound emitters (GOB tag 3). Driven each
    /// frame by [`Pal4Scene::tick_sound_emitters`] which schedules
    /// random replays in `[min_time, max_time]` while the active
    /// leader is within `trigger_distance` XZ of the emitter's
    /// position. The audio engine is mono with no positional API,
    /// so distance gating is binary: in-range → fire, out-of-range
    /// → don't fire (already-playing instances continue).
    pub(crate) sound_emitters: Vec<SceneSoundEmitter>,
    pub(crate) objects_gob: Option<GobFile>,
    pub(crate) events: Vec<EvfEvent>,
    pub(crate) module: Option<Rc<RefCell<ScriptModule>>>,
    pub(crate) triggers: Vec<Rc<SceneEventTrigger>>,
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

/// Per-frame state for one GOB-authored ambient sound emitter (tag 3).
///
/// `trigger_distance_sq` is pre-squared so the per-frame distance
/// gate compares with `Vec3::xz_dist_sq(...)` — no `sqrt` in the
/// per-emitter inner loop.
pub(crate) struct SceneSoundEmitter {
    pub(crate) name: String,
    pub(crate) position: Vec3,
    pub(crate) min_time: f32,
    pub(crate) max_time: f32,
    pub(crate) trigger_distance_sq: f32,
    pub(crate) next_play_in_sec: f32,
}

/// Floor for SOUND mintime/maxtime values. Source data ships with
/// values in the 5–30 s range; this guard exists only to defend
/// against malformed or zero/NaN/negative entries that would
/// otherwise burst-fire `play_sound` every frame.
const MIN_SOUND_INTERVAL_SEC: f32 = 0.1;

/// Fallback `trigger_distance` for SOUND emitters whose entry has
/// an explicit zero (rare in the corpus, but defending against it
/// at load time avoids a per-frame near-zero comparison that would
/// silently disable the emitter).
const DEFAULT_SOUND_TRIGGER_DISTANCE: f32 = 600.0;

/// Sanitise a SOUND emitter's `min_time` / `max_time` parameters to
/// a sane real-time interval. Returns `(min, max)` with both values
/// finite, non-negative, ≥ `MIN_SOUND_INTERVAL_SEC`, and
/// `max >= min`. Exposed at module level so it can be unit-tested
/// independently of any scene load.
pub(crate) fn sanitise_sound_interval(min_time: f32, max_time: f32) -> (f32, f32) {
    fn clean(v: f32) -> f32 {
        if v.is_nan() || v < MIN_SOUND_INTERVAL_SEC {
            MIN_SOUND_INTERVAL_SEC
        } else {
            v
        }
    }
    let min = clean(min_time);
    let max = clean(max_time).max(min);
    (min, max)
}

/// Draw a uniform sample in `[lo, hi]`. Returns `lo` when `hi == lo`
/// (avoids `rand`'s panic on empty ranges) and clamps any inverted
/// inputs.
fn uniform(lo: f32, hi: f32) -> f32 {
    use rand::Rng;
    if hi <= lo {
        lo
    } else {
        rand::thread_rng().gen_range(lo..=hi)
    }
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
            objects_gob_indices: vec![],
            objects_initial_transforms: vec![],
            sound_emitters: vec![],
            objects_gob: None,
            events: vec![],
            module: None,
            triggers: vec![],
            bsp_entity: None,
            floor_entity: None,
            wall_entity: None,
            game_context: None,
            actor_controller: None,
        }
    }

    pub fn load(
        asset_loader: &Rc<asset_loader::AssetLoader>,
        input: Rc<RefCell<dyn InputEngine>>,
        scene_name: &str,
        block_name: &str,
        actor_controller_factory: Option<&ComRc<IPal4ScriptFactory>>,
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
        // drives per-frame UV animation via a self-ticking
        // `UvAnimationComponent` attached to the water entity.
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
                attach_uv_anim(&water, &dict);
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
        let mut objects_gob_indices: Vec<usize> = vec![];
        let mut objects_initial_transforms: Vec<Mat44> = vec![];
        let mut sound_emitters: Vec<SceneSoundEmitter> = vec![];
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

            // Build an ambient sound emitter for SOUND-tag entries; we
            // do this before the visible-entity skip so the emitter
            // bookkeeping is independent of the rendering path. Entries
            // missing any of (name, min_time, max_time) are silently
            // dropped — `tools/pal4_gob_inspect` confirms the corpus
            // ships them all populated.
            if object_type == GobObjectType::SOUND {
                if let (Some(name), Some(min_t), Some(max_t)) = (
                    entry.sound_name(),
                    entry.sound_min_time(),
                    entry.sound_max_time(),
                ) {
                    let (min_time, max_time) = sanitise_sound_interval(min_t, max_t);
                    let trigger_distance = if entry.trigger_distance > 0.0 {
                        entry.trigger_distance
                    } else {
                        DEFAULT_SOUND_TRIGGER_DISTANCE
                    };
                    // First-fire phase ∈ [0, max_time] (NOT [min, max])
                    // staggers dense scenes — many corpus emitters have
                    // identical `(min, max)` so a [min, max] initial
                    // draw lock-steps every emitter in the block.
                    let next_play_in_sec = uniform(0.0, max_time);
                    sound_emitters.push(SceneSoundEmitter {
                        name,
                        position: Vec3::from(entry.position),
                        min_time,
                        max_time,
                        trigger_distance_sq: trigger_distance * trigger_distance,
                        next_play_in_sec,
                    });
                }
            }

            match (object_name, folder, file_name) {
                (Ok(object_name), Ok(folder), Ok(file_name)) => {
                    let entity_name = logical_name.unwrap_or_else(|| object_name.clone());

                    // Skip rendering for the GOB tags that are
                    // non-visual by design — the mesh field on
                    // these entries is just a placement preview the
                    // level editor needed on disk, never a
                    // renderable. Confirmed empirically against the
                    // full PAL4 corpus via `tools/pal4_gob_inspect`:
                    //
                    //   tag 3  SOUND   303/303 entries use placeholder MC
                    //   tag 8  EFFECT  1058/1058 use ZZA/MC/JDumy/H_065
                    //   tag 9  MARKER  57/57 use JDumy/MC
                    //
                    // MARKER entries are now loaded as INVISIBLE
                    // empty entities (no mesh) so script lookups via
                    // `Pal4Scene::get_object(name)` resolve them and
                    // `giGOB*` mutators (set_position, movment,
                    // reset, scale) have something to act on.
                    // SOUND / EFFECT remain skipped — SOUND drives
                    // its own emitter list above, EFFECT needs a
                    // particle subsystem we don't have yet.
                    //
                    // Note: a handful of tag-5 MACHINE entries also
                    // ship placeholder meshes (MC/JDumy) as
                    // state-machine anchors. We intentionally do
                    // **not** filter on mesh name here — most of
                    // those are already covered by the explicit
                    // HIDE flag (4 of 5 corpus-wide `HIDE=1`
                    // entries are MACHINE), and speculatively
                    // hiding the rest risks suppressing real
                    // designer intent. Add a dedicated case if a
                    // specific MACHINE entry surfaces as
                    // incorrectly visible.
                    if matches!(object_type, GobObjectType::EFFECT | GobObjectType::SOUND) {
                        continue;
                    }

                    let entity = if object_type == GobObjectType::MARKER {
                        // Pure script anchor — no mesh, no children.
                        // `set_visible(false)` would normally also
                        // disable rendering for downstream cameras,
                        // but markers don't render anything anyway;
                        // we keep `set_enabled(true)` so scene
                        // updates still tick through the entity (in
                        // case a future feature attaches a component
                        // that needs per-frame work).
                        CoreEntity::create(entity_name.clone(), true)
                    } else {
                        asset_loader
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

                    // Honor the `PAL4-GameObject-object-hide` flag: when set,
                    // the entity is placed in the scene but starts invisible.
                    // Scripts can later reveal it via the `giGOB*` API.
                    // Markers are always invisible — they have no mesh.
                    let initially_hidden =
                        object_type == GobObjectType::MARKER || entry.is_initially_hidden();
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

                    // Snapshot the *full* matrix AFTER the load-time
                    // chain so `giGOBReset` can restore it byte-exact
                    // regardless of any subsequent rotate / scale /
                    // translate composition.
                    let initial_matrix = *entity.transform().borrow().matrix();

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

        // Surface name collisions in the loaded `objects` set:
        // `get_object` returns the first match, so a duplicate
        // silently masks every later same-named entry. Log once at
        // load time so a future regression shows up in the logs
        // instead of being chased through scripts.
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
                        scene_name,
                        block_name
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
            objects_gob_indices,
            objects_initial_transforms,
            sound_emitters,
            objects_gob: Some(gob),
            events: events.events,
            module: Some(module),
            triggers,
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
            // Pre-lockstep this used `entries[i]`, but `objects`
            // skips SOUND / EFFECT (and historically MARKER) so the
            // index could land on the wrong GOB any time a sound /
            // effect appeared before a GENERIC interactable. The
            // lockstep `objects_gob_indices[i]` recovers the
            // authored entry.
            let gob_index = self.objects_gob_indices[i];
            let entry = &self.objects_gob.as_ref().unwrap().entries[gob_index];
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

    /// Index lookup for `objects` by logical name. Returns the
    /// position of the first match (mirrors `get_object`'s
    /// first-match-wins semantics) so callers can index into the
    /// parallel `objects_xz_aabbs` /
    /// `objects_initial_transforms` Vecs.
    fn object_index_by_name(&self, name: &str) -> Option<usize> {
        self.objects.iter().position(|o| o.name() == name)
    }

    /// Set an object's local position to `(x, y, z)` and invalidate
    /// its cached `objects_xz_aabbs` slot so the interaction probe
    /// falls back to anchor-distance for this entry from now on.
    /// Returns `true` on hit, `false` on miss; warns are the
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
        if let Some(slot) = self.objects_xz_aabbs.get_mut(idx) {
            *slot = None;
        }
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
        if let Some(slot) = self.objects_xz_aabbs.get_mut(idx) {
            *slot = None;
        }
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
        if let Some(slot) = self.objects_xz_aabbs.get_mut(idx) {
            *slot = None;
        }
        true
    }

    /// Restore an object's transform to its load-time snapshot in
    /// `objects_initial_transforms`. Invalidates the cached AABB
    /// too — the load-time AABB was sampled against the same
    /// matrix, but recomputing it from the snapshot is more work
    /// than just letting the interaction probe fall back to the
    /// anchor.
    pub fn reset_object(&mut self, name: &str) -> bool {
        let Some(idx) = self.object_index_by_name(name) else {
            return false;
        };
        let saved = self.objects_initial_transforms[idx];
        self.objects[idx].transform().borrow_mut().set_matrix(saved);
        if let Some(slot) = self.objects_xz_aabbs.get_mut(idx) {
            *slot = None;
        }
        true
    }

    /// Per-frame ambient-sound emitter tick. Decrements each
    /// emitter's countdown; on expiry re-rolls a fresh interval
    /// from `[min_time, max_time]` AND, when the leader is within
    /// the emitter's `trigger_distance`, queues the emitter's WAV
    /// name in the returned Vec. The caller (`Pal4AppContext`)
    /// drains the Vec into `play_sound`.
    ///
    /// Timing decisions (see plan.md "Rubber-duck adjustments"):
    /// (1) we re-roll the timer regardless of distance — freezing
    /// would let a long out-of-range stay queue up dozens of
    /// expired timers that all fire on re-entry; (2) the first
    /// fire used a `[0, max_time]` phase rather than `[min, max]`
    /// to stagger dense scenes where every emitter shares the
    /// same period; (3) gating is binary because the audio engine
    /// is mono.
    ///
    /// Active OpenAL sources from prior fires are NOT preempted —
    /// they finish naturally, which matches the engine's mono
    /// behaviour for one-shot ambient sounds.
    pub fn tick_sound_emitters(&mut self, leader_pos: Vec3, delta_sec: f32) -> Vec<String> {
        let mut to_play = Vec::new();
        for emitter in &mut self.sound_emitters {
            emitter.next_play_in_sec -= delta_sec;
            if emitter.next_play_in_sec > 0.0 {
                continue;
            }
            emitter.next_play_in_sec = uniform(emitter.min_time, emitter.max_time);

            let dx = leader_pos.x - emitter.position.x;
            let dz = leader_pos.z - emitter.position.z;
            let dist_sq = dx * dx + dz * dz;
            if dist_sq <= emitter.trigger_distance_sq {
                to_play.push(emitter.name.clone());
            }
        }
        to_play
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_emitter(min: f32, max: f32, trigger_dist: f32, next_play: f32) -> SceneSoundEmitter {
        SceneSoundEmitter {
            name: "WA01".to_string(),
            position: Vec3::new(0.0, 0.0, 0.0),
            min_time: min,
            max_time: max,
            trigger_distance_sq: trigger_dist * trigger_dist,
            next_play_in_sec: next_play,
        }
    }

    fn make_scene_with_emitters(emitters: Vec<SceneSoundEmitter>) -> Pal4Scene {
        let mut s = Pal4Scene::new_empty();
        s.sound_emitters = emitters;
        s
    }

    /// Pin clamping behaviour at the SOUND-emitter boundary: any of
    /// NaN, negative, zero, or near-zero must round-trip to at least
    /// MIN_SOUND_INTERVAL_SEC, and max must never fall below min.
    /// Without this floor a single corrupt entry would burst-fire
    /// `play_sound` every frame.
    #[test]
    fn sanitise_sound_interval_clamps_bad_inputs() {
        assert_eq!(sanitise_sound_interval(5.0, 10.0), (5.0, 10.0));
        // max < min → clamp max up to min
        assert_eq!(sanitise_sound_interval(10.0, 3.0), (10.0, 10.0));
        // zero / negative → floored
        let (lo, hi) = sanitise_sound_interval(0.0, 0.0);
        assert!(lo >= MIN_SOUND_INTERVAL_SEC && hi >= MIN_SOUND_INTERVAL_SEC && hi >= lo);
        let (lo, hi) = sanitise_sound_interval(-5.0, -1.0);
        assert!(lo >= MIN_SOUND_INTERVAL_SEC && hi >= MIN_SOUND_INTERVAL_SEC && hi >= lo);
        // NaN
        let (lo, hi) = sanitise_sound_interval(f32::NAN, f32::NAN);
        assert!(lo >= MIN_SOUND_INTERVAL_SEC && hi >= MIN_SOUND_INTERVAL_SEC && hi >= lo);
    }

    /// In-range emitter whose timer expires this tick must surface
    /// in the returned Vec exactly once.
    #[test]
    fn sound_emitter_fires_when_leader_in_range() {
        let mut s = make_scene_with_emitters(vec![make_emitter(5.0, 5.0, 600.0, 0.05)]);
        let to_play = s.tick_sound_emitters(Vec3::new(10.0, 0.0, 10.0), 0.1);
        assert_eq!(to_play, vec!["WA01".to_string()]);
        // Timer was re-rolled (min==max=5s, so exactly 5s).
        assert!((s.sound_emitters[0].next_play_in_sec - 5.0).abs() < 1e-4);
    }

    /// Out-of-range emitter still re-rolls its timer on expiry — if
    /// we froze the timer instead, all expired emitters would burst-
    /// fire the moment the leader re-entered the area.
    #[test]
    fn sound_emitter_silent_when_leader_out_of_range_but_timer_resets() {
        let mut s = make_scene_with_emitters(vec![make_emitter(5.0, 5.0, 600.0, 0.05)]);
        // 1000 XZ units away with trigger_distance = 600.
        let to_play = s.tick_sound_emitters(Vec3::new(1000.0, 0.0, 0.0), 0.1);
        assert!(to_play.is_empty(), "out of range, must not fire");
        assert!(
            (s.sound_emitters[0].next_play_in_sec - 5.0).abs() < 1e-4,
            "timer must reset"
        );
    }

    /// Emitter whose timer is still in flight must not fire — the
    /// per-frame countdown is the only source of new plays.
    #[test]
    fn sound_emitter_does_not_fire_before_expiry() {
        let mut s = make_scene_with_emitters(vec![make_emitter(5.0, 5.0, 600.0, 1.0)]);
        let to_play = s.tick_sound_emitters(Vec3::new(0.0, 0.0, 0.0), 0.1);
        assert!(to_play.is_empty());
        // Timer ticked down but did not reset.
        assert!((s.sound_emitters[0].next_play_in_sec - 0.9).abs() < 1e-4);
    }

    /// Vec3-distance-squared comparison correctness: a leader sitting
    /// exactly on the trigger boundary should be considered in range
    /// (≤, not <), and a hair beyond should not.
    #[test]
    fn sound_emitter_distance_boundary_is_inclusive() {
        let mut s = make_scene_with_emitters(vec![make_emitter(5.0, 5.0, 100.0, 0.0)]);
        // Exactly on the boundary
        let to_play = s.tick_sound_emitters(Vec3::new(100.0, 0.0, 0.0), 0.0);
        assert_eq!(to_play.len(), 1, "boundary must be inclusive");

        let mut s = make_scene_with_emitters(vec![make_emitter(5.0, 5.0, 100.0, 0.0)]);
        // Just beyond
        let to_play = s.tick_sound_emitters(Vec3::new(100.1, 0.0, 0.0), 0.0);
        assert!(to_play.is_empty(), "just beyond must not fire");
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
        s.objects.push(entity);
        s.objects_gob_indices.push(0);
        s.objects_initial_transforms.push(snap);
        s.objects_xz_aabbs.push(None);

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
        s.objects_xz_aabbs.push(None);

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

    /// AABB cache must be invalidated after any transform mutation
    /// so `test_interaction` does not consult a stale rectangle for
    /// a moved interactable.
    #[test]
    fn aabb_is_invalidated_after_transform_mutators() {
        let mut s = Pal4Scene::new_empty();
        let entity = CoreEntity::create("crate001".to_string(), true);
        let snap = *entity.transform().borrow().matrix();
        s.objects.push(entity);
        s.objects_gob_indices.push(0);
        s.objects_initial_transforms.push(snap);
        s.objects_xz_aabbs.push(Some((0.0, 1.0, 0.0, 1.0)));

        assert!(s.set_object_position("crate001", 5.0, 0.0, 5.0));
        assert!(
            s.objects_xz_aabbs[0].is_none(),
            "AABB must be cleared on set_position"
        );

        s.objects_xz_aabbs[0] = Some((0.0, 1.0, 0.0, 1.0));
        assert!(s.set_object_position_and_yaw("crate001", 0.0, 0.0, 0.0, 30.0));
        assert!(
            s.objects_xz_aabbs[0].is_none(),
            "AABB must be cleared on movement"
        );

        s.objects_xz_aabbs[0] = Some((0.0, 1.0, 0.0, 1.0));
        assert!(s.set_object_scale_xy("crate001", 2.0, 2.0));
        assert!(
            s.objects_xz_aabbs[0].is_none(),
            "AABB must be cleared on scale"
        );

        s.objects_xz_aabbs[0] = Some((0.0, 1.0, 0.0, 1.0));
        assert!(s.reset_object("crate001"));
        assert!(
            s.objects_xz_aabbs[0].is_none(),
            "AABB must be cleared on reset"
        );
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
