use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use crosscom::ComRc;
use fileformats::pal4::cam::CameraDataFile;
use radiance::{
    audio::{AudioEngine, AudioMemorySource, AudioSourceState},
    comdef::{IEntity, IEntityExt, ISceneExt, ISceneManager},
    input::InputEngine,
    math::{Transform, Vec3},
    radiance::{TaskManager, UiManager},
    rendering::{ComponentFactory, VideoPlayer},
    utils::{act_drop::ActDrop, interp_value::InterpValue},
};

use crate::ui::dialog_box::{AvatarPosition, DialogBox};

/// Dependency-free dialog snapshot used by external observers
/// (debug overlays, the agent server). Avoids forcing every reader of
/// `Pal4AppContext` to import imgui-tied dialog types.
#[derive(Debug, Clone)]
pub struct DialogStateSnapshot {
    pub open: bool,
    pub text: String,
    pub avatar: DialogAvatarSide,
}

/// Multiplier applied to the per-frame movement / rotation tween dt
/// when `Pal4AppContext::fast_forward()` is true. The value is large
/// enough that the per-frame step always exceeds the remaining
/// distance / angle so the snap-to-target paths inside
/// `update_moving_entities_` / `update_rotating_entities` fire on
/// the first tick after `npc_to` / `player_walk_to` enqueues the
/// move — making wait-for-motion script continuations
/// (`npc_end_move`, `player_end_move`, `player_set_dir { sync = 1 }`,
/// …) effectively zero-cost under fast-forward.
const MOTION_FAST_FORWARD_SCALE: f32 = 1_000.0;

/// Which side the dialog avatar portrait is currently anchored to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogAvatarSide {
    Left,
    Right,
}

/// Per-slot party snapshot used by `Pal4AppContext::party_snapshot`.
#[derive(Debug, Clone, Default)]
pub struct PartySnapshot {
    pub slot: usize,
    pub level: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub in_team: bool,
}

use super::{
    actor::{IPal4ActorAnimationControllerExt, Pal4ActorAnimation, Pal4ActorAnimationConfig},
    asset_loader::AssetLoader,
    scene::{Pal4ActorControllerFactory, Pal4Scene},
    states::persistent_state::{PAL4_APP_NAME, Pal4PersistentState},
};

pub struct Pal4AppContext {
    pub(crate) loader: Rc<AssetLoader>,
    pub(crate) scene_manager: ComRc<ISceneManager>,
    pub(crate) ui: Rc<UiManager>,
    pub(crate) input: Rc<RefCell<dyn InputEngine>>,
    pub(crate) task_manager: Rc<TaskManager>,
    pub(crate) scene: Pal4Scene,
    pub(crate) dialog_box: DialogBox,

    component_factory: Rc<dyn ComponentFactory>,
    audio_engine: Rc<dyn AudioEngine>,
    video_player: Box<VideoPlayer>,
    /// Currently playing background music. The engine auto-ticks the
    /// underlying source via its weak-source list, so we only need to
    /// keep the handle alive here. Dropping it (in `stop_bgm` / on
    /// `play_bgm` re-issue) immediately tears down the OpenAL source —
    /// this is what guarantees we can't stack overlapping BGMs.
    bgm_source: Option<Box<dyn AudioMemorySource>>,
    /// Live one-shot sound effects (`gi2DSoundPlay`). Each slot owns
    /// its own source; the slot is reclaimed when the source reports
    /// `Stopped` (auto-prune at the start of `play_sound`) or when the
    /// script explicitly stops it via `gi2DSoundStop[ID]`.
    sound_sources: HashMap<i32, Box<dyn AudioMemorySource>>,
    sound_id: i32,
    actdrop: ActDrop,
    /// Active voice line. Same lifetime contract as `bgm_source` —
    /// dropping the handle stops the voice immediately, so
    /// fast-forwarding through a dialog run can't stack voice samples.
    voice_source: Option<Box<dyn AudioMemorySource>>,
    camera_data: Option<CameraDataFile>,
    camera_run: Option<CameraRun>,
    scene_name: String,
    block_name: String,
    leader: usize,
    player_locked: bool,

    /// Plot fast-forward toggle, driven by the PAL4 debug overlay.
    /// `Cell` so the `&self` script continuations (giWait / giTalk /
    /// camera waits) can read it while the director writes it via
    /// `set_fast_forward` once per frame.
    fast_forward: Cell<bool>,

    moving_entities: HashMap<ActorId, MovingEntity>,
    rotating_entities: HashMap<ActorId, RotatingEntity>,

    /// Serializable PAL4 game progress (money, party, inventory,
    /// story flags). Mutated by the scripted game-state functions in
    /// `openpal4::scripting` and persisted by the director's
    /// save/load orchestration.
    persistent_state: Pal4PersistentState,

    /// Factory for the scripted `IPal4ActorController` component. `None`
    /// until the application bootstrap calls
    /// [`set_actor_controller_factory`]; when `None`, `load_scene`
    /// loads scenes without per-player controllers (e.g. before the
    /// script project is installed).
    actor_controller_factory: Option<Rc<dyn Pal4ActorControllerFactory>>,

    /// Items the active script has queued for the next
    /// `giShowSelectDialog` / `giShowCommonDialogInSelectMode`.
    /// Populated by `giSelectDialogAddItem`; cleared when the
    /// matching `giSelectDialogGetLastSelect` /
    /// `giCommonDialogGetLastSelect` returns the choice.
    ///
    /// Surfaced via `/v1/state.dialog.choices` so the planner can
    /// see the available options before picking via
    /// `/v1/dialog/choose`.
    pending_dialog_choices: Vec<String>,
    /// Choice index to return from the next `*_get_last_select`
    /// call. `None` ≡ "use the default (1)" — the legacy stubbed
    /// behaviour. Consumed (taken) on the next read so each fire
    /// must set it again.
    next_dialog_choice: Cell<Option<i32>>,
    /// `true` while a `giShowWorldMap` continuation is waiting for a
    /// destination pick. Surfaced via `/v1/state.world_map_open` so
    /// the planner knows it must `POST /v1/world_map/choose` before
    /// the script can advance. Cleared when the buffered choice is
    /// consumed by the continuation.
    world_map_open: Cell<bool>,
    /// Buffered world-map destination as `(scene, block)`. Set by
    /// `/v1/world_map/choose`, consumed by the `giShowWorldMap`
    /// continuation which then performs the equivalent of
    /// `giArenaLoad(scene, block, "", 0)`. `None` ≡ "no choice yet".
    world_map_choice: RefCell<Option<(String, String)>>,
}

impl Pal4AppContext {
    pub fn new(
        component_factory: Rc<dyn ComponentFactory>,
        loader: Rc<AssetLoader>,
        scene_manager: ComRc<ISceneManager>,
        ui: Rc<UiManager>,
        input: Rc<RefCell<dyn InputEngine>>,
        audio_engine: Rc<dyn AudioEngine>,
        task_manager: Rc<TaskManager>,
    ) -> Self {
        Self {
            loader,
            scene_manager,
            ui: ui.clone(),
            task_manager,
            input,
            component_factory: component_factory.clone(),
            audio_engine,
            video_player: component_factory.create_video_player(),
            bgm_source: None,
            sound_sources: HashMap::new(),
            sound_id: 0,
            actdrop: ActDrop::new(),
            voice_source: None,
            camera_data: None,
            camera_run: None,
            scene_name: String::new(),
            block_name: String::new(),
            leader: 0,
            scene: Pal4Scene::new_empty(),
            dialog_box: DialogBox::new(ui),
            player_locked: true,
            fast_forward: Cell::new(false),
            moving_entities: HashMap::new(),
            rotating_entities: HashMap::new(),
            actor_controller_factory: None,
            persistent_state: Pal4PersistentState::new(PAL4_APP_NAME.to_string()),
            pending_dialog_choices: Vec::new(),
            next_dialog_choice: Cell::new(None),
            world_map_open: Cell::new(false),
            world_map_choice: RefCell::new(None),
        }
    }

    /// Install the scripted `IPal4ActorController` factory. Subsequent
    /// `load_scene` calls hand the factory to `Pal4Scene::load`, which
    /// attaches a freshly minted controller component to each player
    /// entity. Idempotent; replaces any previous factory.
    pub fn set_actor_controller_factory(&mut self, factory: Rc<dyn Pal4ActorControllerFactory>) {
        self.actor_controller_factory = Some(factory);
    }

    pub fn update(&mut self, delta_sec: f32) {
        let _timer = radiance::perf::timer("pal4.app_context.update_total_ns");
        radiance::perf::gauge(
            "pal4.app_context.moving_entities",
            self.moving_entities.len() as u64,
        );
        radiance::perf::gauge(
            "pal4.app_context.rotating_entities",
            self.rotating_entities.len() as u64,
        );
        self.actdrop.update(self.ui.ui(), delta_sec);
        // When fast-forward is active we accelerate the movement /
        // rotation tweens so wait-for-motion script continuations
        // (player_end_move, npc_end_move, player_set_dir { sync = 1 },
        // etc.) finish in a single tick. The scale is chosen large
        // enough that the per-frame step always exceeds the
        // remaining distance / angle, triggering the snap-to-target
        // path in the inner loops.
        //
        // Camera runs and UV animations stay on real time so visuals
        // don't go pathological if a fast-forwarded scene is paused
        // partway through (the planner can still see the visual
        // state in `/v1/screenshot`).
        let motion_dt = if self.fast_forward.get() {
            delta_sec * MOTION_FAST_FORWARD_SCALE
        } else {
            delta_sec
        };
        self.update_moving_entities(motion_dt);
        self.update_rotating_entities(motion_dt);
        self.tick_camera_run(delta_sec);

        // Tick ambient SOUND emitters (GOB tag 3). We use real
        // `delta_sec` rather than `motion_dt` deliberately: fast-
        // forward is a debug / planner convenience that compresses
        // wait loops, but if we scaled audio retrigger intervals
        // by the same factor every ambient emitter in the scene
        // would burst-fire on each fast-forwarded frame. Fire-and-
        // forget plays land in `sound_sources` so `gi2DSoundStop` (→
        // `stop_all_sounds`) still tears them down for scripted
        // SFX cleanup. A WAV that started just before a scene swap
        // continues to play on its OpenAL source until it finishes
        // — same carry-over behaviour as the existing music /
        // voice paths.
        let leader_pos = self
            .scene
            .get_player(self.leader)
            .world_transform()
            .position();
        let to_play = self.scene.tick_sound_emitters(leader_pos, delta_sec);
        for name in to_play {
            if let Err(e) = self.play_sound(&name) {
                log::warn!("ambient sound emitter '{}' failed: {:#}", name, e);
            }
        }
    }

    fn update_moving_entities(&mut self, delta_sec: f32) {
        let moving_entities = std::mem::take(&mut self.moving_entities);
        self.moving_entities = self.update_moving_entities_(moving_entities, delta_sec);
    }

    fn update_moving_entities_(
        &mut self,
        mut entities: HashMap<ActorId, MovingEntity>,
        delta_sec: f32,
    ) -> HashMap<ActorId, MovingEntity> {
        let mut to_remove = Vec::new();

        const RUN_SPEED: f32 = 150.;
        const WALK_SPEED: f32 = 75.;

        for (id, entity) in entities.iter() {
            let pos = entity.entity.transform().borrow().position();
            let target = entity.target;
            let speed = if entity.run { RUN_SPEED } else { WALK_SPEED };

            let moving_distance = speed * delta_sec;
            let diff = Vec3::sub(&target, &pos);
            let distance = diff.norm();
            if distance < moving_distance {
                entity.entity.transform().borrow_mut().set_position(&target);
                to_remove.push(id.clone());
            } else {
                let direction = Vec3::normalized(&diff);
                let new_pos = Vec3::add(&pos, &Vec3::scalar_mul(moving_distance, &direction));
                let look_at = Vec3::new(pos.x, new_pos.y, pos.z);
                entity
                    .entity
                    .transform()
                    .borrow_mut()
                    .set_position(&new_pos)
                    .look_at(&look_at);
            }
        }

        for id in to_remove {
            match &id {
                ActorId::Player(player) => {
                    self.player_play_animation(*player as i32, Pal4ActorAnimation::Idle);
                }
                ActorId::Npc(name) => {
                    self.npc_play_animation(name, Pal4ActorAnimation::Idle);
                }
            }

            entities.remove(&id);
        }

        entities
    }

    fn update_rotating_entities(&mut self, delta_sec: f32) {
        // PAL4 cutscene turns feel natural at about half a turn per second.
        const ROTATE_DEG_PER_SEC: f32 = 180.0;

        if self.rotating_entities.is_empty() {
            return;
        }

        let step = ROTATE_DEG_PER_SEC * delta_sec;
        let entities = std::mem::take(&mut self.rotating_entities);
        let mut to_finish = Vec::new();
        let mut kept = HashMap::new();

        for (id, mut rot) in entities.into_iter() {
            let delta = wrap_deg(rot.target_deg - rot.current_deg);
            let snap = delta.abs() <= step.max(0.0001);
            rot.current_deg = if snap {
                rot.target_deg
            } else {
                rot.current_deg + step.copysign(delta)
            };

            // `look_at` orients the entity so its forward (matrix column 2)
            // equals `pos - target`. To face direction `(sin yaw, 0, cos yaw)`
            // — matching what `set_player_ang(yaw)` produces via
            // `rotate_axis_angle_local(UP, yaw)` — the look_at target must
            // be `pos - (sin yaw, 0, cos yaw)`.
            let pos = rot.entity.transform().borrow().position();
            let yaw_rad = rot.current_deg.to_radians();
            let target = Vec3::new(pos.x - yaw_rad.sin(), pos.y, pos.z - yaw_rad.cos());
            rot.entity
                .transform()
                .borrow_mut()
                .set_position(&pos)
                .look_at(&target);

            if snap {
                to_finish.push(id);
            } else {
                kept.insert(id, rot);
            }
        }

        self.rotating_entities = kept;

        for id in to_finish {
            match &id {
                ActorId::Player(player) => {
                    self.player_play_animation(*player as i32, Pal4ActorAnimation::Idle);
                }
                ActorId::Npc(name) => {
                    self.npc_play_animation(name, Pal4ActorAnimation::Idle);
                }
            }
        }
    }

    pub fn player_rotate_to(&mut self, player: i32, target_deg: f32) {
        let mapped = self.map_player(player);
        let entity = self.scene.get_player(mapped);

        let current_deg = yaw_from_transform(&entity);
        self.player_play_animation(player, Pal4ActorAnimation::Walk);
        self.rotating_entities.insert(
            ActorId::Player(mapped),
            RotatingEntity {
                entity,
                current_deg,
                target_deg,
            },
        );
    }

    pub fn player_rotating(&self, player: i32) -> bool {
        let mapped = self.map_player(player);
        self.rotating_entities
            .contains_key(&ActorId::Player(mapped))
    }

    pub fn npc_rotate_to(&mut self, name: &str, target_deg: f32) {
        let Some(entity) = self.scene.get_npc(name) else {
            return;
        };

        let current_deg = yaw_from_transform(&entity);
        self.npc_play_animation(name, Pal4ActorAnimation::Walk);
        self.rotating_entities.insert(
            ActorId::Npc(name.to_string()),
            RotatingEntity {
                entity,
                current_deg,
                target_deg,
            },
        );
    }

    pub fn npc_rotating(&self, name: &str) -> bool {
        self.rotating_entities
            .contains_key(&ActorId::Npc(name.to_string()))
    }

    pub fn event_triggered(&mut self, _delta_sec: f32) -> Option<String> {
        self.scene
            .test_event_triggers()
            .and_then(|event| event.function.function.to_string().ok())
            .or_else(|| self.scene.test_interaction(self.input.clone(), self.leader))
    }

    pub fn set_actdrop(&mut self, darkness: InterpValue<f32>) {
        self.actdrop.set_darkness(darkness);
    }

    pub fn get_actdrop(&self) -> &ActDrop {
        &self.actdrop
    }

    pub fn set_leader(&mut self, leader: i32) {
        self.leader = leader as usize;
        self.enable_player(self.leader, true);
        // Route the (single) per-scene actor controller to the new
        // leader's entity. Without this, the previous leader's
        // controller would keep ticking floor/wall raycasts against
        // its own (now hidden) entity — manifesting as "invisible
        // walls" or "phasing through walls" after a leader switch.
        self.scene.set_active_leader(self.leader);
    }

    pub fn set_player_pos(&mut self, player: i32, pos: &Vec3) {
        let player = self.map_player(player);
        self.enable_player(player, true);

        self.scene
            .get_player(player)
            .transform()
            .borrow_mut()
            .set_position(&pos);
    }

    pub fn enable_player(&mut self, player: usize, enable: bool) {
        let player = self.scene.get_player(player);
        player.set_visible(enable);
        player.set_enabled(enable);
    }

    pub fn enable_npc(&mut self, npc: &str, enable: bool) {
        let npc = self.scene.get_npc(npc);
        if let Some(npc) = npc {
            npc.set_visible(enable);
            npc.set_enabled(enable);
        }
    }

    pub fn enable_object(&mut self, object: &str, enable: bool) {
        let object = self.scene.get_object(object);
        if let Some(object) = object {
            object.set_visible(enable);
            object.set_enabled(enable);
        }
    }

    pub fn get_player_pos(&mut self, player: i32) -> Vec3 {
        let player = self.map_player(player);

        self.scene
            .get_player(player)
            .transform()
            .borrow()
            .position()
    }

    pub fn player_to(&mut self, player: i32, target: &Vec3, run: bool) {
        let mapped_player = self.map_player(player);
        let entity = self.scene.get_player(mapped_player);

        let moving_entity = MovingEntity {
            entity,
            target: target.clone(),
            run,
        };

        let animation = if run {
            Pal4ActorAnimation::Run
        } else {
            Pal4ActorAnimation::Walk
        };

        self.player_play_animation(player, animation);
        self.moving_entities
            .insert(ActorId::Player(mapped_player), moving_entity);
    }

    pub fn player_moving(&mut self, player: i32) -> bool {
        let player = self.map_player(player);
        self.moving_entities.contains_key(&ActorId::Player(player))
    }

    pub fn npc_to(&mut self, name: &str, target: &Vec3, run: bool) {
        let entity = self.scene.get_npc(name);
        if entity.is_none() {
            return;
        }

        let moving_entity = MovingEntity {
            entity: entity.unwrap(),
            target: target.clone(),
            run,
        };

        let animation = if run {
            Pal4ActorAnimation::Run
        } else {
            Pal4ActorAnimation::Walk
        };

        self.npc_play_animation(name, animation);
        self.moving_entities
            .insert(ActorId::Npc(name.to_string()), moving_entity);
    }

    pub fn npc_moving(&mut self, name: &str) -> bool {
        self.moving_entities
            .contains_key(&ActorId::Npc(name.to_string()))
    }

    pub fn player_lookat(&mut self, player: i32, target: &Vec3) {
        let player = self.map_player(player);

        self.scene
            .get_player(player)
            .transform()
            .borrow_mut()
            .look_at(target);
    }

    /// Yaw-only look-at for an actor: ignores the vertical component
    /// so actors don't tilt up/down when "facing" another actor whose
    /// pivot is at a different height. Used by the `giPlayerFaceTo*`
    /// / `giNpcFaceTo*` script functions.
    pub fn face_player_to_pos(&mut self, player: i32, target: &Vec3) {
        let player = self.map_player(player);
        let entity = self.scene.get_player(player);
        let pos = entity.transform().borrow().position();
        let look_at = Vec3::new(target.x, pos.y, target.z);
        entity.transform().borrow_mut().look_at(&look_at);
    }

    pub fn face_npc_to_pos(&mut self, name: &str, target: &Vec3) {
        if let Some(entity) = self.scene.get_npc(name) {
            let pos = entity.transform().borrow().position();
            let look_at = Vec3::new(target.x, pos.y, target.z);
            entity.transform().borrow_mut().look_at(&look_at);
        }
    }

    pub fn npc_pos(&self, name: &str) -> Option<Vec3> {
        self.scene
            .get_npc(name)
            .map(|e| e.transform().borrow().position())
    }

    pub fn npc_set_pos(&mut self, name: &str, pos: &Vec3) {
        if let Some(entity) = self.scene.get_npc(name) {
            entity.transform().borrow_mut().set_position(pos);
        }
    }

    pub fn npc_set_ang(&mut self, name: &str, ang: f32) {
        if let Some(entity) = self.scene.get_npc(name) {
            entity
                .transform()
                .borrow_mut()
                .clear_rotation()
                .rotate_axis_angle_local(&Vec3::UP, ang.to_radians());
        }
    }

    /// Resolve the position of either a player slot (0-3, or -1 for the
    /// current leader) or — used by the few script functions that lump
    /// player/npc anchors together — fall back to `(0,0,0)`.
    pub fn camera_position(&self) -> Vec3 {
        self.scene_manager
            .scene()
            .map(|s| s.camera().transform().position())
            .unwrap_or_else(|| Vec3::new(0.0, 0.0, 0.0))
    }

    /// Camera orientation in degrees (`Vec3 { x: pitch, y: yaw, z: roll }`).
    pub fn camera_euler_deg(&self) -> Vec3 {
        self.scene_manager
            .scene()
            .map(|s| s.camera().transform().euler())
            .unwrap_or_else(|| Vec3::new(0.0, 0.0, 0.0))
    }

    /// Full camera transform (position + orientation) of the active
    /// scene, or `None` when no scene is loaded. Used by save-load to
    /// snapshot the exact view.
    pub fn camera_transform(&self) -> Option<Transform> {
        self.scene_manager
            .scene()
            .map(|s| s.camera().transform().clone())
    }

    /// Restore the active scene camera to `transform`. No-op when no
    /// scene is loaded. The gameplay camera is static between cinematic
    /// camera runs, so this faithfully reinstates a saved view.
    pub fn set_camera_transform(&mut self, transform: &Transform) {
        if let Some(scene) = self.scene_manager.scene() {
            scene
                .camera_mut()
                .transform_mut()
                .set_matrix(transform.matrix().clone());
        }
    }

    pub fn lock_player(&mut self, lock: bool) {
        self.player_locked = lock;
        if let Some(ctrl) = self.scene.actor_controller() {
            ctrl.lock_control(lock);
        }
    }

    /// Whether player input control is currently locked (e.g. during a
    /// scripted cutscene). Snapshotted by save-load so a restored game
    /// resumes with the same controllability.
    pub fn is_player_locked(&self) -> bool {
        self.player_locked
    }

    pub fn set_player_ang(&mut self, player: i32, ang: f32) {
        let player = self.map_player(player);

        self.scene
            .get_player(player)
            .transform()
            .borrow_mut()
            .clear_rotation()
            .rotate_axis_angle_local(&Vec3::UP, ang.to_radians());
    }

    pub fn player_do_action(&mut self, player: i32, action: &str, flag: i32) {
        let player = self.map_player(player);
        let metadata = self.scene.get_player_metadata(player);
        let anm = self.loader.load_anm(metadata.actor_name(), action).unwrap();
        let events = self.loader.load_amf(metadata.actor_name(), action);

        let config = match flag {
            -1 => Pal4ActorAnimationConfig::PauseOnHold,
            0 => Pal4ActorAnimationConfig::Looping,

            // TODO: >0 means playing n times
            _ => Pal4ActorAnimationConfig::OneTime,
        };

        self.scene
            .get_player_controller(player)
            .play_animation(anm, events, config);
    }

    pub fn player_play_animation(&mut self, player: i32, animation: Pal4ActorAnimation) {
        let player = self.map_player(player);
        self.scene
            .get_player_controller(player)
            .play(animation, Pal4ActorAnimationConfig::Looping);
    }

    pub fn npc_play_animation(&mut self, name: &str, animation: Pal4ActorAnimation) {
        self.scene
            .get_npc_controller(name)
            .map(|controller| controller.play(animation, Pal4ActorAnimationConfig::Looping));
    }

    pub fn player_unhold_act(&mut self, player: i32) {
        let player = self.map_player(player);
        self.scene.get_player_controller(player).unhold();
    }

    pub fn player_act_completed(&mut self, player: i32) -> bool {
        let player = self.map_player(player);
        self.scene
            .get_player_controller(player)
            .animation_completed()
    }

    pub fn player_set_direction(&mut self, player: i32, direction: f32) {
        let player = self.map_player(player);
        self.scene
            .get_player(player)
            .transform()
            .borrow_mut()
            .clear_rotation()
            .rotate_axis_angle_local(&Vec3::UP, direction * std::f32::consts::PI / 180.0);
    }

    /// Neutral, dependency-free snapshot of the currently displayed
    /// dialog. Consumed by the agent-server adapter (and any future
    /// debug UI) without forcing every reader to import the imgui
    /// `DialogBox` type. The `text` field is the **markup-stripped**
    /// visible form (PAL4 `<colour>` / `<dcN>` tags removed), so
    /// `/v1/state.dialog.text` consumers don't need to parse the raw
    /// CEGUI markup themselves.
    pub fn dialog_snapshot(&self) -> DialogStateSnapshot {
        DialogStateSnapshot {
            open: self.dialog_box.is_active(),
            text: self.dialog_box.text().to_string(),
            avatar: match self.dialog_box.avatar_position() {
                AvatarPosition::Left => DialogAvatarSide::Left,
                AvatarPosition::Right => DialogAvatarSide::Right,
            },
        }
    }

    /// `&self`-safe accessor for the currently playing movie state.
    /// Returns `true` while a `play_movie()` script call has the video
    /// player in `Playing`/`Paused`, `false` once it has stopped (or no
    /// movie has been played yet). Surfaced through the agent
    /// `/v1/state` snapshot so external drivers can wait on movies.
    pub fn movie_playing(&self) -> bool {
        use radiance::video::VideoStreamState;
        self.video_player.get_state() != VideoStreamState::Stopped
    }

    /// Per-slot party snapshot (level/HP/MP/in-team). Returned in
    /// `slot`-ascending order so consumers can index by position.
    pub fn party_snapshot(&self) -> Vec<PartySnapshot> {
        let mut out = Vec::with_capacity(crate::openpal4::states::persistent_state::PLAYER_COUNT);
        for slot in 0..crate::openpal4::states::persistent_state::PLAYER_COUNT {
            let p = self
                .persistent_state
                .player(slot)
                .cloned()
                .unwrap_or_default();
            out.push(PartySnapshot {
                slot,
                level: p.level,
                hp: p.hp,
                max_hp: p.max_hp,
                mp: p.mp,
                max_mp: p.max_mp,
                in_team: p.in_team,
            });
        }
        out
    }

    /// Snapshot of the player's inventory as `(equipment_id, count)`
    /// pairs. Sorted by `equipment_id` for deterministic client
    /// rendering; an empty `Vec` here is the canonical "no items"
    /// state, not an error.
    pub fn inventory_snapshot(&self) -> Vec<(i32, i32)> {
        let mut out: Vec<(i32, i32)> = self
            .persistent_state
            .inventory_iter()
            .map(|(id, count)| (*id, *count))
            .collect();
        out.sort_by_key(|(id, _)| *id);
        out
    }

    /// Items queued for the next select-dialog. See the field doc
    /// on [`Self::pending_dialog_choices`] for the lifecycle.
    pub fn dialog_choices(&self) -> &[String] {
        &self.pending_dialog_choices
    }

    /// Append one item to the pending dialog-choice list. Called
    /// from the `giSelectDialogAddItem` sysfn handler.
    pub fn push_dialog_choice(&mut self, item: String) {
        self.pending_dialog_choices.push(item);
    }

    /// Buffer a choice for the next `*_get_last_select` call.
    /// Wired to `/v1/dialog/choose`. The value is consumed on the
    /// next read (so each pick lasts for exactly one dialog).
    pub fn buffer_dialog_choice(&self, index: i32) {
        self.next_dialog_choice.set(Some(index));
    }

    /// Take the buffered choice (or default to `1`) and clear the
    /// pending-items list. Called from the `*_get_last_select`
    /// sysfn handlers.
    pub fn take_dialog_choice(&mut self) -> i32 {
        let choice = self.next_dialog_choice.take().unwrap_or(1);
        self.pending_dialog_choices.clear();
        choice
    }

    /// `true` while a `giShowWorldMap` continuation is waiting for a
    /// destination pick. Surfaced via `/v1/state.world_map_open` so
    /// external drivers know they must `POST /v1/world_map/choose`
    /// before the script can advance.
    pub fn world_map_open(&self) -> bool {
        self.world_map_open.get()
    }

    /// Mark the world map as open (called by `giShowWorldMap`'s
    /// continuation entry). Idempotent.
    pub fn open_world_map(&self) {
        self.world_map_open.set(true);
    }

    /// Buffer a `(scene, block)` destination for the next world-map
    /// continuation tick. Wired to `/v1/world_map/choose`. Consumed
    /// on the next tick — agents must re-supply a choice for each
    /// `giShowWorldMap` prompt.
    pub fn buffer_world_map_choice(&self, scene: String, block: String) {
        *self.world_map_choice.borrow_mut() = Some((scene, block));
    }

    /// Take the buffered world-map choice (if any) and mark the
    /// world map closed. Returning `None` ≡ "still waiting, keep
    /// looping". Called from the `giShowWorldMap` continuation.
    pub fn take_world_map_choice(&self) -> Option<(String, String)> {
        let choice = self.world_map_choice.borrow_mut().take();
        if choice.is_some() {
            self.world_map_open.set(false);
        }
        choice
    }

    pub fn scene_name(&self) -> &str {
        &self.scene_name
    }

    pub fn block_name(&self) -> &str {
        &self.block_name
    }

    pub fn leader(&self) -> usize {
        self.leader
    }

    pub fn persistent_state(&self) -> &Pal4PersistentState {
        &self.persistent_state
    }

    pub fn persistent_state_mut(&mut self) -> &mut Pal4PersistentState {
        &mut self.persistent_state
    }

    /// Overwrite the entire persistent state (used when loading a
    /// save slot). Caller is responsible for any follow-up scene
    /// reload / leader restore.
    pub fn set_persistent_state(&mut self, state: Pal4PersistentState) {
        self.persistent_state = state;
    }

    /// Forward the PAL4 debug overlay's BSP-visibility toggle to the
    /// current `Pal4Scene`. The director calls this each frame with
    /// the script-supplied flag, so scene reloads pick up the latest
    /// state without extra wiring.
    pub fn set_bsp_visible(&mut self, visible: bool) {
        self.scene.set_bsp_visible(visible);
    }

    /// Same idea as [`Pal4AppContext::set_bsp_visible`] but for the
    /// floor + wall nav-mesh overlay geometry.
    pub fn set_nav_mesh_visible(&mut self, visible: bool) {
        self.scene.set_nav_mesh_visible(visible);
    }

    /// Push the PAL4 debug overlay's plot fast-forward toggle. The
    /// director fans this in each frame; the script wait/dialog/camera
    /// continuations read it via [`Pal4AppContext::fast_forward`] to
    /// short-circuit to completion.
    pub fn set_fast_forward(&mut self, fast_forward: bool) {
        self.fast_forward.set(fast_forward);
    }

    /// `&self`-safe read of the plot fast-forward toggle, used from
    /// inside the script global-function continuations (which only
    /// borrow `app_context` immutably).
    pub fn fast_forward(&self) -> bool {
        self.fast_forward.get()
    }

    /// `&self`-safe leader position lookup for diagnostics overlays.
    /// Returns `Vec3::new(0.0, 0.0, 0.0)` while the scene hasn't been
    /// loaded (e.g. before the first `load_scene` call).
    pub fn leader_pos(&self) -> Vec3 {
        let leader = self.leader;
        // `Pal4Scene::get_player` is `&self`. On an empty scene the
        // helper still returns the placeholder entity slot, whose
        // transform reports the identity translation — fine for the
        // debug overlay to render zeros.
        self.scene
            .get_player(leader)
            .transform()
            .borrow()
            .position()
    }

    /// Leader facing direction in degrees, matching the convention of
    /// `player_set_direction` (yaw about world-up). Used by save-load
    /// to restore the exact orientation the player was standing in.
    pub fn leader_direction(&self) -> f32 {
        yaw_from_transform(&self.scene.get_player(self.leader))
    }

    pub fn load_scene(&mut self, scene_name: &str, block_name: &str) -> anyhow::Result<()> {
        let _ = self.scene_manager.pop_scene();
        let scene = Pal4Scene::load(
            &self.loader,
            self.input.clone(),
            scene_name,
            block_name,
            self.actor_controller_factory.as_ref(),
        )?;
        self.scene = scene;
        self.scene_manager.push_scene(self.scene.scene.clone());

        self.scene_name = scene_name.to_string();
        self.block_name = block_name.to_string();

        self.set_leader(self.leader as i32);
        self.lock_player(self.player_locked);
        Ok(())
    }

    pub fn start_play_movie(&mut self, name: &str) -> Option<(u32, u32)> {
        let reader = self.loader.load_video(name).unwrap();
        self.video_player.play(
            self.component_factory.clone(),
            self.audio_engine.clone(),
            reader,
            radiance::video::Codec::Bik,
            false,
        )
    }

    pub fn video_player(&mut self) -> &mut VideoPlayer {
        &mut self.video_player
    }

    pub fn play_bgm(&mut self, name: &str) -> anyhow::Result<()> {
        // Drop the previous source before allocating the new one, so
        // even if the load below fails we still have stopped the old
        // track and we never hold two BGM sources simultaneously.
        self.stop_bgm();

        let data = self.loader.load_music(name)?;
        let mut source = self.audio_engine.create_source();
        source.set_data(data, radiance::audio::Codec::Mp3);
        source.play(true);

        self.bgm_source = Some(source);

        Ok(())
    }

    pub fn stop_bgm(&mut self) {
        if let Some(mut source) = self.bgm_source.take() {
            source.stop();
        }
    }

    pub fn play_sound(&mut self, name: &str) -> anyhow::Result<i32> {
        // Reclaim slots whose WAV has finished playing on its own so
        // ambient SOUND emitters that fire every few seconds don't
        // grow `sound_sources` unboundedly.
        self.sound_sources
            .retain(|_, s| s.state() != AudioSourceState::Stopped);

        let id = self.find_next_sound_id();
        let source = self.play_sound_internal(name, radiance::audio::Codec::Wav)?;
        self.sound_sources.insert(id, source);
        Ok(id)
    }

    pub fn stop_sound(&mut self, id: i32) {
        if let Some(mut source) = self.sound_sources.remove(&id) {
            source.stop();
        }
    }

    pub fn stop_all_sounds(&mut self) {
        for (_, mut source) in self.sound_sources.drain() {
            source.stop();
        }
    }

    pub fn play_voice(&mut self, name: &str) -> anyhow::Result<()> {
        self.stop_voice();

        let source = self.play_sound_internal(name, radiance::audio::Codec::Mp3)?;
        self.voice_source = Some(source);
        Ok(())
    }

    pub fn stop_voice(&mut self) {
        if let Some(mut source) = self.voice_source.take() {
            source.stop();
        }
    }

    pub fn prepare_camera(&mut self, name: &str) -> anyhow::Result<()> {
        let data = self
            .loader
            .load_camera_data(name, &self.scene_name, &self.block_name)?;
        self.camera_data = Some(data);
        Ok(())
    }

    pub fn run_camera(&mut self, name: &str) {
        self.start_camera_run(name);
    }

    /// Begin a (possibly multi-frame) camera animation. Returns true if an
    /// async animation is now in flight; false if the camera was snapped
    /// (instant flag, missing data, or fewer than 2 keyframes).
    pub fn start_camera_run(&mut self, name: &str) -> bool {
        log::debug!("start_camera_run: {}", name);
        let Some(data) = self.camera_data.as_ref() else {
            return false;
        };
        let Some(camera_data) = data.get_camera_data(name) else {
            log::warn!("Requested camera data '{}' not found", name);
            return false;
        };

        let look_at_arr = camera_data.get_look_at();
        let look_at = Vec3::new(look_at_arr[0], look_at_arr[1], look_at_arr[2]);
        let mut keyframes: Vec<Vec3> = camera_data
            .keyframes()
            .into_iter()
            .map(|k| Vec3::new(k[0], k[1], k[2]))
            .collect();

        let snap_to = |ctx: &Pal4AppContext, pos: Vec3| {
            if let Some(scene) = ctx.scene_manager.scene() {
                scene
                    .camera_mut()
                    .transform_mut()
                    .set_position(&pos)
                    .look_at(&look_at);
            }
        };

        if keyframes.is_empty() {
            self.camera_run = None;
            return false;
        }

        let raw_duration = camera_data.duration();

        if camera_data.is_instant() || raw_duration <= 0.0 {
            snap_to(self, *keyframes.last().unwrap());
            self.camera_run = None;
            return false;
        }

        // PAL4 cam entries often record only the destination keyframe; in that
        // case treat the camera's current position as the implicit start.
        if keyframes.len() < 2 {
            let current_pos = self
                .scene_manager
                .scene()
                .map(|s| s.camera_mut().transform_mut().position())
                .unwrap_or_else(|| Vec3::new(0.0, 0.0, 0.0));
            keyframes.insert(0, current_pos);
        }

        // Build per-segment arc lengths along the polyline.
        let mut segment_lengths = Vec::with_capacity(keyframes.len() - 1);
        let mut total_length = 0.0_f32;
        for w in keyframes.windows(2) {
            let len = Vec3::sub(&w[1], &w[0]).norm();
            segment_lengths.push(len);
            total_length += len;
        }

        let duration = raw_duration;

        log::debug!(
            "start_camera_run: name={} keyframes={} total_len={:.2} duration={:.2}s instant={} debug_fields={:?}",
            name,
            keyframes.len(),
            total_length,
            duration,
            camera_data.is_instant(),
            camera_data.debug_fields()
        );

        // Snap look-at to target immediately and place the camera at the
        // start of the polyline so the lerp animates visibly.
        if let Some(scene) = self.scene_manager.scene() {
            scene
                .camera_mut()
                .transform_mut()
                .set_position(&keyframes[0])
                .look_at(&look_at);
        }

        self.camera_run = Some(CameraRun {
            waypoints: keyframes,
            segment_lengths,
            total_length,
            look_at,
            elapsed: 0.0,
            duration,
        });
        true
    }

    pub fn camera_running(&self) -> bool {
        self.camera_run.is_some()
    }

    fn tick_camera_run(&mut self, delta_sec: f32) {
        let Some(run) = self.camera_run.as_mut() else {
            return;
        };

        run.elapsed += delta_sec;
        let last = *run.waypoints.last().unwrap();
        let look_at = run.look_at;

        let position = if run.elapsed >= run.duration || run.total_length <= 0.0 {
            last
        } else {
            // Walk segments until we find the one containing the current arc length.
            let target_len = (run.elapsed / run.duration) * run.total_length;
            let mut acc = 0.0_f32;
            let mut pos = last;
            for (i, seg_len) in run.segment_lengths.iter().enumerate() {
                if target_len <= acc + *seg_len || i == run.segment_lengths.len() - 1 {
                    let local = if *seg_len > 0.0 {
                        ((target_len - acc) / *seg_len).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let a = run.waypoints[i];
                    let b = run.waypoints[i + 1];
                    pos = Vec3::new(
                        a.x + (b.x - a.x) * local,
                        a.y + (b.y - a.y) * local,
                        a.z + (b.z - a.z) * local,
                    );
                    break;
                }
                acc += *seg_len;
            }
            pos
        };

        if let Some(scene) = self.scene_manager.scene() {
            scene
                .camera_mut()
                .transform_mut()
                .set_position(&position)
                .look_at(&look_at);
        }

        if run.elapsed >= run.duration {
            self.camera_run = None;
        }
    }

    pub fn set_portrait(&mut self, name: &str, left: bool) {
        let image = self.loader.load_portrait(name);
        self.dialog_box.set_avatar(
            image,
            if left {
                AvatarPosition::Left
            } else {
                AvatarPosition::Right
            },
        );
    }

    fn find_next_sound_id(&mut self) -> i32 {
        while self.sound_sources.contains_key(&self.sound_id) {
            self.sound_id += 1;
            if self.sound_id == 10000 {
                self.sound_id = 0;
            }
        }

        self.sound_id
    }

    fn play_sound_internal(
        &mut self,
        name: &str,
        codec: radiance::audio::Codec,
    ) -> anyhow::Result<Box<dyn AudioMemorySource>> {
        let ext = if codec == radiance::audio::Codec::Mp3 {
            "mp3"
        } else {
            "wav"
        };

        let data = self.loader.load_sound(name, ext)?;
        let mut source = self.audio_engine.create_source();
        source.set_data(data, codec);
        source.play(false);

        Ok(source)
    }

    #[inline]
    fn map_player(&self, player: i32) -> usize {
        if player == -1 {
            self.leader
        } else {
            player as usize
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum ActorId {
    Player(usize),
    Npc(String),
}

struct MovingEntity {
    entity: ComRc<IEntity>,
    target: Vec3,
    run: bool,
}

struct RotatingEntity {
    entity: ComRc<IEntity>,
    current_deg: f32,
    target_deg: f32,
}

struct CameraRun {
    waypoints: Vec<Vec3>,
    segment_lengths: Vec<f32>,
    total_length: f32,
    look_at: Vec3,
    elapsed: f32,
    duration: f32,
}

/// Wrap an angular delta in degrees into the (-180, 180] range so we always
/// rotate via the shortest arc.
fn wrap_deg(mut d: f32) -> f32 {
    while d > 180.0 {
        d -= 360.0;
    }
    while d <= -180.0 {
        d += 360.0;
    }
    d
}

/// Recover the yaw (degrees) that `look_at(pos + (sin yaw, 0, cos yaw))`
/// would produce, by reading the forward column of the transform matrix.
/// `Transform::euler()` does NOT return yaw in `.y` — its Y component is
/// the X-axis rotation in this codebase's decomposition.
fn yaw_from_transform(entity: &ComRc<IEntity>) -> f32 {
    let t = entity.transform();
    let m = t.borrow();
    let mat = m.matrix();
    let fx = mat[0][2];
    let fz = mat[2][2];
    fx.atan2(fz).to_degrees()
}
