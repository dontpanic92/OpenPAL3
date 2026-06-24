//! PAL5 story-runtime command context (game-specific Rust).
//!
//! Holds all engine handles + script state the PAL5 Lua command
//! handlers need, and implements the handlers themselves. The Lua
//! bridge (the `extern "C"` trampolines + registration + harness) lives
//! in [`super::commands`]; the per-frame driver in [`super::director`].
//!
//! This is the "functional bootstrap" surface: the essentials (scene
//! load, NPC/player create+place, static/lerp camera, fades,
//! dialog/print, best-effort audio) are implemented; the rest (battle,
//! item/magic grants, patrol AI, anim chains) are logged no-ops so the
//! `NewGame -> m001_1` intro runs end-to-end without erroring.

use std::cell::RefCell;
use std::collections::HashMap;
use std::os::raw::c_char;
use std::rc::Rc;

use crosscom::ComRc;
use encoding::{DecoderTrap, Encoding};
use radiance::audio::{AudioEngine, AudioMemorySource, AudioSourceState};
use radiance::comdef::{IEntity, IEntityExt, ISceneManager};
use radiance::input::{InputEngine, Key};
use radiance::math::Vec3;
use radiance::radiance::UiManager;
use radiance::rendering::ComponentFactory;
use radiance::utils::act_drop::ActDrop;
use radiance::utils::interp_value::InterpValue;

use shared::openpal5::asset_loader::AssetLoader;
use shared::openpal5::scene::Pal5Scene;
use shared::openpal5::script::ScriptIndex;

/// The single scene name the bootstrap loads for the first segment.
/// Map-id → scene-name resolution (`map.xml` / `MapInfo.ini`) is a
/// follow-up; the狂风寨 intro only needs this one scene.
const BOOTSTRAP_SCENE: &str = "kuangfengzhai";

struct CameraLerp {
    from_eye: Vec3,
    from_look: Vec3,
    to_eye: Vec3,
    to_look: Vec3,
    duration: f32,
    elapsed: f32,
}

struct Dialog {
    name: Option<String>,
    text: String,
}

pub struct Pal5ScriptContext {
    asset_loader: Rc<AssetLoader>,
    script_index: Rc<ScriptIndex>,
    scene_manager: ComRc<ISceneManager>,
    component_factory: Rc<dyn ComponentFactory>,
    audio_engine: Rc<dyn AudioEngine>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
    ui: Rc<UiManager>,

    scene: Option<Pal5Scene>,
    scene_loaded: bool,

    flags: HashMap<i32, i32>,
    npcs: HashMap<i32, ComRc<IEntity>>,
    players: HashMap<i32, ComRc<IEntity>>,

    cam_eye: Vec3,
    cam_look: Vec3,
    pending_lerp_ms: f32,
    lerp: Option<CameraLerp>,

    actdrop: ActDrop,
    dialog: Option<Dialog>,
    anykey_latch: bool,

    bgm: Box<dyn AudioMemorySource>,
    sounds: Vec<RefCell<Box<dyn AudioMemorySource>>>,

    sleep_sec: f32,
    finished: bool,
}

impl Pal5ScriptContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        asset_loader: Rc<AssetLoader>,
        script_index: Rc<ScriptIndex>,
        scene_manager: ComRc<ISceneManager>,
        component_factory: Rc<dyn ComponentFactory>,
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
        ui: Rc<UiManager>,
    ) -> Self {
        let bgm = audio_engine.create_source();
        Self {
            asset_loader,
            script_index,
            scene_manager,
            component_factory,
            audio_engine,
            input_engine,
            ui,
            scene: None,
            scene_loaded: false,
            flags: HashMap::new(),
            npcs: HashMap::new(),
            players: HashMap::new(),
            cam_eye: Vec3::new(0.0, 0.0, 0.0),
            cam_look: Vec3::new(0.0, 0.0, 1.0),
            pending_lerp_ms: 0.0,
            lerp: None,
            actdrop: ActDrop::new(),
            dialog: None,
            anykey_latch: false,
            bgm,
            sounds: Vec::new(),
            sleep_sec: 0.0,
            finished: false,
        }
    }

    // ---- driver-facing helpers -----------------------------------

    pub fn script_index(&self) -> &Rc<ScriptIndex> {
        &self.script_index
    }

    pub fn asset_loader(&self) -> &Rc<AssetLoader> {
        &self.asset_loader
    }

    pub fn set_sleep(&mut self, sec: f32) {
        self.sleep_sec = sec.max(0.0);
        self.anykey_latch = false;
    }

    pub fn is_sleeping(&self) -> bool {
        self.sleep_sec > 0.0
    }

    pub fn mark_finished(&mut self) {
        self.finished = true;
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn camera_lerp_remaining(&self) -> f32 {
        self.lerp
            .as_ref()
            .map(|l| (l.duration - l.elapsed).max(0.0))
            .unwrap_or(0.0)
    }

    /// Current scene name for the agent snapshot — empty until the
    /// bootstrap scene has been loaded.
    pub fn current_scene_name(&self) -> String {
        if self.scene_loaded {
            BOOTSTRAP_SCENE.to_string()
        } else {
            String::new()
        }
    }

    /// Leader (player 1) world position for the agent snapshot, if the
    /// player entity has been created.
    pub fn leader_position(&self) -> Option<[f32; 3]> {
        let entity = self.players.get(&1)?;
        let pos = entity.transform().borrow().position();
        Some([pos.x, pos.y, pos.z])
    }

    /// Agent fast-forward tick: collapse any pending `Wait` sleep and
    /// dismiss the current dialog so the Lua VM resumes immediately
    /// this frame. Mirrors PAL3's SCE fast-forward, which skips
    /// `giWait` / dialog waits.
    pub fn fast_forward_skip(&mut self) {
        self.sleep_sec = 0.0;
        self.dialog = None;
    }

    /// Per-frame update (runs every frame, even while sleeping).
    pub fn update(&mut self, delta_sec: f32) {
        if self.is_sleeping() {
            self.sleep_sec -= delta_sec;
            // A keypress fast-forwards the current Wait (skip dialog).
            if self.anykey_pressed() {
                self.sleep_sec = 0.0;
            }
        }

        self.update_camera_lerp(delta_sec);
        self.actdrop.update(self.ui.ui(), delta_sec);
        self.update_audio();
        self.update_dialog();
    }

    fn update_camera_lerp(&mut self, delta_sec: f32) {
        let Some(lerp) = self.lerp.as_mut() else {
            return;
        };
        lerp.elapsed += delta_sec;
        let t = if lerp.duration > 0.0 {
            (lerp.elapsed / lerp.duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let eye = lerp_vec3(&lerp.from_eye, &lerp.to_eye, t);
        let look = lerp_vec3(&lerp.from_look, &lerp.to_look, t);
        let done = t >= 1.0;
        self.apply_camera(eye, look);
        if done {
            self.lerp = None;
        }
    }

    fn update_audio(&mut self) {
        for s in &self.sounds {
            s.borrow_mut().update();
        }
        self.sounds
            .retain(|s| s.borrow().state() != AudioSourceState::Stopped);
        self.bgm.update();
    }

    fn update_dialog(&mut self) {
        if self.anykey_latch {
            self.dialog = None;
        }
        let ui = self.ui.ui();
        if let Some(dialog) = &self.dialog {
            let [w, h] = ui.io().display_size;
            ui.window("pal5_dialog")
                .position([w * 0.5 - 360.0, h - 200.0], imgui::Condition::Always)
                .size([720.0, -1.0], imgui::Condition::Always)
                .movable(false)
                .resizable(false)
                .collapsible(false)
                .title_bar(false)
                .build(|| {
                    if let Some(name) = &dialog.name {
                        ui.text_colored([1.0, 0.85, 0.4, 1.0], name.as_str());
                    }
                    ui.text_wrapped(dialog.text.as_str());
                });
        }
    }

    fn apply_camera(&mut self, eye: Vec3, look: Vec3) {
        if let Some(cam) = self.scene_manager.camera() {
            cam.set_position(eye.x, eye.y, eye.z);
            cam.look_at(look.x, look.y, look.z);
        }
        self.cam_eye = eye;
        self.cam_look = look;
    }

    fn anykey_pressed(&self) -> bool {
        let input = self.input_engine.borrow();
        input.get_key_state(Key::Space).pressed()
            || input.get_key_state(Key::Escape).pressed()
            || input.get_key_state(Key::GamePadSouth).pressed()
    }

    fn ensure_scene(&mut self) {
        if self.scene_loaded {
            return;
        }
        self.scene_loaded = true;
        match Pal5Scene::load(&self.asset_loader, BOOTSTRAP_SCENE) {
            Ok(scene) => {
                self.scene_manager.push_scene(scene.scene.clone());
                self.scene = Some(scene);
                log::info!("PAL5: loaded bootstrap scene '{}'", BOOTSTRAP_SCENE);
            }
            Err(e) => log::error!("PAL5: failed to load scene '{}': {}", BOOTSTRAP_SCENE, e),
        }
    }

    fn spawn_model(&self, model_id: i32, x: f32, z: f32) -> Option<ComRc<IEntity>> {
        let item = self.asset_loader.index.get(&(model_id as u32))?;
        let file_path = item.file_path.to_string();
        if !file_path.ends_with(".dff") {
            return None;
        }
        match self.asset_loader.load_model(&file_path) {
            Ok(entity) => {
                entity
                    .transform()
                    .borrow_mut()
                    .set_position(&Vec3::new(x, 0.0, z));
                if let Some(scene) = &self.scene {
                    scene.scene.add_entity(entity.clone());
                }
                Some(entity)
            }
            Err(e) => {
                log::warn!("PAL5: load model {} ({}): {}", model_id, file_path, e);
                None
            }
        }
    }

    // ---- command handlers: global --------------------------------

    pub fn global_print(&mut self, text: *const c_char) {
        log::info!("[PAL5 script] {}", decode_gbk(text));
    }

    pub fn global_begin_scene(&mut self, _scene_id: f64) {
        self.ensure_scene();
    }

    pub fn global_end_scene(&mut self) {}

    pub fn global_set_wide_screen(&mut self, _on: f64) {}

    pub fn global_play_music(&mut self, music_id: f64, _vol: f64) {
        let id = music_id as i32;
        if id < 0 {
            self.bgm.stop();
            return;
        }
        // Best-effort: PAL5 music lives in Music.pkg as pNN.smp/mp3;
        // wiring the loader is a follow-up. Logged for now.
        log::info!("PAL5: PlayMusic({})", id);
    }

    pub fn global_get_music_id(&mut self) -> f64 {
        0.0
    }

    pub fn global_music_fade_in(&mut self, _ms: f64) {}

    pub fn global_music_fade_out(&mut self, _ms: f64) {
        self.bgm.stop();
    }

    pub fn global_play_sound(&mut self, sound_id: f64, _vol: f64) {
        log::debug!("PAL5: PlaySound({})", sound_id as i32);
    }

    pub fn global_stop_last_sound(&mut self) {}

    pub fn global_play_cg(&mut self, cg_id: f64) {
        // CG/movie playback is best-effort; skipped for the bootstrap.
        log::info!("PAL5: PlayCg({}) (skipped)", cg_id as i32);
    }

    // ---- command handlers: flag ----------------------------------

    pub fn flag_set_value(&mut self, flag: f64, value: f64) {
        self.flags.insert(flag as i32, value as i32);
    }

    pub fn flag_get_value(&mut self, flag: f64) -> f64 {
        *self.flags.get(&(flag as i32)).unwrap_or(&0) as f64
    }

    // ---- command handlers: player --------------------------------

    pub fn player_create(&mut self, role_id: f64, x: f64, z: f64) {
        if let Some(e) = self.spawn_model(role_id as i32, x as f32, z as f32) {
            self.players.insert(role_id as i32, e);
        }
    }

    pub fn player_set_pos(&mut self, x: f64, z: f64) {
        // Player 1 is the leader; reposition every known player entity.
        for e in self.players.values() {
            e.transform()
                .borrow_mut()
                .set_position(&Vec3::new(x as f32, 0.0, z as f32));
        }
    }

    pub fn player_set_visible(&mut self, role_id: f64, visible: f64) {
        if let Some(e) = self.players.get(&(role_id as i32)) {
            e.set_visible(visible != 0.0);
        }
    }

    pub fn player_remove(&mut self, role_id: f64) {
        if let Some(e) = self.players.remove(&(role_id as i32)) {
            e.set_visible(false);
        }
    }

    pub fn player_is_in_team(&mut self, _role_id: f64) -> f64 {
        1.0
    }

    pub fn player_get_item_count(&mut self, _item_id: f64) -> f64 {
        0.0
    }

    // ---- command handlers: npc -----------------------------------

    pub fn npc_create(&mut self, model_id: f64, handle: f64, x: f64, z: f64) {
        if let Some(e) = self.spawn_model(model_id as i32, x as f32, z as f32) {
            self.npcs.insert(handle as i32, e);
        }
    }

    pub fn npc_set_pos(&mut self, handle: f64, x: f64, z: f64) {
        if let Some(e) = self.npcs.get(&(handle as i32)) {
            e.transform()
                .borrow_mut()
                .set_position(&Vec3::new(x as f32, 0.0, z as f32));
        }
    }

    pub fn npc_set_pos_3d(&mut self, handle: f64, x: f64, y: f64, z: f64) {
        if let Some(e) = self.npcs.get(&(handle as i32)) {
            e.transform()
                .borrow_mut()
                .set_position(&Vec3::new(x as f32, y as f32, z as f32));
        }
    }

    pub fn npc_move_to(&mut self, handle: f64, x: f64, z: f64) {
        // Movement snaps instantly for the bootstrap (WaitForNpcPos is
        // then immediately satisfied).
        self.npc_set_pos(handle, x, z);
    }

    pub fn npc_run_to(&mut self, handle: f64, x: f64, z: f64) {
        self.npc_set_pos(handle, x, z);
    }

    pub fn npc_set_visible(&mut self, handle: f64, visible: f64) {
        if let Some(e) = self.npcs.get(&(handle as i32)) {
            e.set_visible(visible != 0.0);
        }
    }

    pub fn npc_destroy(&mut self, handle: f64) {
        if let Some(e) = self.npcs.remove(&(handle as i32)) {
            e.set_visible(false);
        }
    }

    pub fn npc_is_created(&mut self, handle: f64) -> f64 {
        self.npcs.contains_key(&(handle as i32)) as i32 as f64
    }

    // ---- command handlers: camera --------------------------------

    pub fn camera_change_static(&mut self, ex: f64, ey: f64, ez: f64, lx: f64, ly: f64, lz: f64) {
        let eye = Vec3::new(ex as f32, ey as f32, ez as f32);
        let look = Vec3::new(lx as f32, ly as f32, lz as f32);
        if self.pending_lerp_ms > 0.0 {
            self.lerp = Some(CameraLerp {
                from_eye: self.cam_eye,
                from_look: self.cam_look,
                to_eye: eye,
                to_look: look,
                duration: self.pending_lerp_ms / 1000.0,
                elapsed: 0.0,
            });
            self.pending_lerp_ms = 0.0;
        } else {
            self.lerp = None;
            self.apply_camera(eye, look);
        }
    }

    pub fn camera_change_static_eye(
        &mut self,
        ex: f64,
        ey: f64,
        ez: f64,
        lx: f64,
        ly: f64,
        lz: f64,
    ) {
        self.camera_change_static(ex, ey, ez, lx, ly, lz);
    }

    pub fn camera_reset_lerp(&mut self, ms: f64) {
        self.pending_lerp_ms = ms as f32;
    }

    /// Place the camera at an absolute pose (used by the agent server's
    /// `/v1/camera/pose`). Cancels any in-flight lerp and pending-lerp
    /// request so the pose is not immediately animated away. Stable only
    /// while the debug camera is enabled (plot frozen); otherwise the
    /// next scripted camera command will overwrite it.
    pub fn set_camera_pose(&mut self, eye: Vec3, look: Vec3) {
        self.lerp = None;
        self.pending_lerp_ms = 0.0;
        self.apply_camera(eye, look);
    }

    /// Current camera pose `(eye, look)` for the agent state snapshot.
    pub fn camera_pose(&self) -> (Vec3, Vec3) {
        (self.cam_eye, self.cam_look)
    }

    // ---- command handlers: effect --------------------------------

    pub fn effect_fade_in(&mut self, _arg: f64, speed: f64) {
        // FadeIn = reveal the scene (darkness 1 -> 0).
        let sp = (speed as f32).max(0.05);
        self.actdrop.set_darkness(InterpValue::new(1.0, 0.0, sp));
    }

    pub fn effect_fade_out(&mut self, _arg: f64, speed: f64) {
        let sp = (speed as f32).max(0.05);
        self.actdrop.set_darkness(InterpValue::new(0.0, 1.0, sp));
    }

    // ---- command handlers: ui ------------------------------------

    pub fn ui_dialog(&mut self, text: *const c_char) {
        self.dialog = Some(Dialog {
            name: None,
            text: decode_gbk(text),
        });
    }

    pub fn ui_message(&mut self, text: *const c_char) {
        self.ui_dialog(text);
    }

    pub fn ui_close_start_menu(&mut self) {}

    // ---- command handlers: map -----------------------------------

    pub fn map_change_no_script(&mut self, _map_id: f64, _sub_id: f64) {
        self.ensure_scene();
    }

    pub fn map_get_current_map_id(&mut self) -> f64 {
        1.0
    }
}

fn lerp_vec3(a: &Vec3, b: &Vec3, t: f32) -> Vec3 {
    Vec3::new(
        a.x + (b.x - a.x) * t,
        a.y + (b.y - a.y) * t,
        a.z + (b.z - a.z) * t,
    )
}

/// Decode a GBK C string from the Lua side into a Rust `String`.
pub fn decode_gbk(s: *const c_char) -> String {
    if s.is_null() {
        return String::new();
    }
    let cstr = unsafe { std::ffi::CStr::from_ptr(s) };
    encoding::all::GBK
        .decode(cstr.to_bytes(), DecoderTrap::Replace)
        .unwrap_or_else(|_| String::from_utf8_lossy(cstr.to_bytes()).into_owned())
}
