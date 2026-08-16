use std::{cell::RefCell, collections::HashMap, os::raw::c_char, rc::Rc};

use crosscom::ComRc;
use encoding::{DecoderTrap, Encoding};
use imgui::{Image, TextureId};
use lua50_32_sys::lua_State;
use radiance::{
    audio::{AudioEngine, AudioMemorySource, AudioSourceState, Codec},
    comdef::ISceneManager,
    input::{InputEngine, Key},
    radiance::UiManager,
    rendering::{ComponentFactory, Sprite, VideoPlayer},
    utils::{act_drop::ActDrop, interp_value::InterpValue},
};

use crate::scripting::lua50_32::Lua5032Vm;

use super::{asset_loader::AssetLoader, scene::Swd5Scene};

pub struct SWD5Context {
    asset_loader: Rc<AssetLoader>,
    audio_engine: Rc<dyn AudioEngine>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
    component_factory: Rc<dyn ComponentFactory>,
    ui: Rc<UiManager>,
    video_player: Box<VideoPlayer>,
    scene_manager: ComRc<ISceneManager>,
    scene: Option<Swd5Scene>,
    sleep_sec: f32,
    /// Current map id (set by `chang_map`). Surfaced to the agent
    /// snapshot as the `scene` field. `0` before the first map load.
    current_map_id: i32,

    bgm_source: Box<dyn AudioMemorySource>,
    sound_sources: HashMap<i32, RefCell<Box<dyn AudioMemorySource>>>,
    story_msg: Option<StoryMsg>,
    story_pic: Option<Sprite>,
    talk_msg: Option<TalkMsg>,

    movie_texture: Option<TextureId>,
    actdrop: ActDrop,
    anykey_down: bool,
}

impl SWD5Context {
    pub fn new(
        asset_loader: Rc<AssetLoader>,
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
        component_factory: Rc<dyn ComponentFactory>,
        scene_manager: ComRc<ISceneManager>,
        ui: Rc<UiManager>,
    ) -> Self {
        let bgm_source = audio_engine.create_source();
        let video_player = component_factory.create_video_player();
        Self {
            asset_loader,
            audio_engine,
            input_engine,
            component_factory,
            ui,
            video_player,
            scene_manager,
            scene: None,
            sleep_sec: 0.,
            current_map_id: 0,
            bgm_source,
            sound_sources: HashMap::new(),
            story_msg: None,
            story_pic: None,
            talk_msg: None,
            movie_texture: None,
            actdrop: ActDrop::new(),
            anykey_down: false,
        }
    }

    pub fn sleep(&mut self, sleep_sec: f32) {
        self.sleep_sec = sleep_sec;
        self.anykey_down = false;
    }

    pub fn is_sleeping(&self) -> bool {
        self.sleep_sec > 0.
    }

    /// Current map id, exposed to the agent-server state snapshot.
    pub fn current_map_id(&self) -> i32 {
        self.current_map_id
    }

    /// Whether a bik movie is currently playing — surfaced to the
    /// agent snapshot's `movie_playing` flag.
    pub fn is_movie_playing(&self) -> bool {
        self.video_player.get_state() == radiance::video::VideoStreamState::Playing
    }

    /// Live dialog text as `(speaker, text)`, or `None` when no
    /// message box is up. A `talkmsg` (which carries a speaker name)
    /// takes precedence over a plain `storymsg`, matching the draw
    /// order in `update`. Surfaced to the agent snapshot's `dialog`
    /// field so a driver can read the line before tapping through it.
    pub fn dialog_text(&self) -> Option<(String, String)> {
        if let Some(talk) = self.talk_msg.as_ref() {
            return Some((talk.name.clone(), talk.text.clone()));
        }

        self.story_msg
            .as_ref()
            .map(|story| (String::new(), story.text.clone()))
    }

    /// Whether a full-screen story picture (`openstorypic`) is
    /// currently displayed.
    pub fn story_pic_open(&self) -> bool {
        self.story_pic.is_some()
    }

    /// Current camera pose as `(eye, look_at)` in world space, or
    /// `None` before the first map load. Surfaced to the agent
    /// snapshot's `camera_eye` / `camera_target`.
    pub fn camera_pose(&self) -> Option<([f32; 3], [f32; 3])> {
        self.scene.as_ref().map(|scene| {
            let eye = scene.camera_position;
            let target = scene.camera_look_at;
            ([eye.x, eye.y, eye.z], [target.x, target.y, target.z])
        })
    }

    /// Agent-driven camera placement: set the eye position and the
    /// look-at target in one shot. Returns `false` when no scene is
    /// loaded yet (the dispatcher turns that into a `Conflict`).
    ///
    /// Note the ordering — `set_camera_lookat` must run first because
    /// `set_camera_pos` re-aims at the stored `camera_look_at`.
    pub fn agent_set_camera(&mut self, eye: [f32; 3], target: [f32; 3]) -> bool {
        let scene = match self.scene.as_mut() {
            Some(scene) => scene,
            None => return false,
        };

        scene.set_camera_lookat(target[0], target[1], target[2]);
        scene.set_camera_pos(eye[0], eye[1], eye[2]);
        true
    }

    /// Agent-driven map change. Reuses the exact `chang_map` path the
    /// Lua VM takes (load + scene-manager pop/push), but reports
    /// failure to the caller instead of only logging it.
    ///
    /// SWDHC has no role entities, so a map id is the only navigable
    /// "position" the agent surface can offer; `/v1/player/teleport`
    /// is routed here.
    pub fn agent_change_map(&mut self, map_id: i32) -> anyhow::Result<()> {
        let scene = Swd5Scene::load(&self.asset_loader, map_id)?;
        self.scene_manager.pop_scene();
        self.scene_manager.push_scene(scene.scene.clone());
        self.scene = Some(scene);
        self.current_map_id = map_id;
        Ok(())
    }

    /// Host functions `/v1/script/eval` is allowed to invoke. Every
    /// entry is a pure state mutation that is safe to run outside a VM
    /// tick; nothing here resumes or inspects the Lua coroutine.
    ///
    /// Keep in sync with the `match` in [`Self::agent_eval`].
    pub const EVAL_ALLOW_LIST: &'static [&'static str] = &[
        "chang_map",
        "story_music",
        "story_music_off",
        "play_sound",
        "stop_sound",
        "play_movie",
        "openstorypic",
        "closestorypic",
        "dark",
        "undark",
    ];

    /// Invoke one of the allow-listed host functions by name with
    /// `f64` arguments, mirroring the Lua ABI (every scripted SWD5
    /// builtin takes numbers). Backs `/v1/script/eval`.
    ///
    /// Deliberately does **not** re-enter the Lua VM: the coroutine is
    /// suspended mid-`sleep` and resuming it out of band would corrupt
    /// the script's control flow. These call straight into the same
    /// `&mut self` methods the registered C shims call.
    pub fn agent_eval(&mut self, function: &str, args: &[f64]) -> Result<(), String> {
        fn arg(args: &[f64], i: usize) -> f64 {
            args.get(i).copied().unwrap_or(0.)
        }

        if !Self::EVAL_ALLOW_LIST.contains(&function) {
            return Err(eval_rejection_message(function));
        }

        match function {
            "chang_map" => self.chang_map(arg(args, 0), arg(args, 1), arg(args, 2), arg(args, 3)),
            "story_music" => self.story_music(
                arg(args, 0),
                arg(args, 1),
                arg(args, 2),
                arg(args, 3),
                arg(args, 4),
                arg(args, 5),
            ),
            "story_music_off" => self.story_music_off(arg(args, 0), arg(args, 1)),
            "play_sound" => self.play_sound(arg(args, 0), arg(args, 1)),
            "stop_sound" => self.stop_sound(arg(args, 0)),
            "play_movie" => self.play_movie(arg(args, 0)),
            "openstorypic" => self.openstorypic(arg(args, 0)),
            "closestorypic" => self.closestorypic(),
            "dark" => self.dark(arg(args, 0)),
            "undark" => self.undark(arg(args, 0)),
            // Unreachable: the allow-list check above already filtered
            // every name not handled here.
            other => return Err(eval_rejection_message(other)),
        }

        Ok(())
    }

    /// Agent fast-forward tick: collapse any pending `sleep` and
    /// dismiss the current story / talk message so the Lua VM resumes
    /// immediately this frame instead of waiting on a scripted pause
    /// or a player keypress. Mirrors PAL3's SCE fast-forward, which
    /// skips `giWait` / dialog waits.
    pub fn fast_forward_skip(&mut self) {
        self.sleep_sec = 0.;
        self.story_msg = None;
        self.talk_msg = None;
    }

    pub fn update(&mut self, delta_sec: f32) {
        if self.is_sleeping() {
            self.sleep_sec -= delta_sec;
            self.anykey_down = self.anykey_down || self.anykey_down();
        }

        self.actdrop.update(self.ui.ui(), delta_sec);

        self.update_audio();
        self.update_story_pic();
        self.update_storymsg();
        self.update_talkmsg();
        self.update_video();
    }

    fn update_storymsg(&mut self) {
        if self.anykey_down() {
            self.story_msg = None;
        }

        let ui = self.ui.ui();
        if let Some(story_msg) = &self.story_msg {
            ui.window("story_msg")
                .position(story_msg.position, imgui::Condition::Always)
                .size([-1., -1.], imgui::Condition::Always)
                .movable(false)
                .resizable(false)
                .collapsible(false)
                .title_bar(false)
                .draw_background(false)
                .build(|| {
                    ui.text(story_msg.text.as_str());
                });
        }
    }

    fn update_talkmsg(&mut self) {
        if self.anykey_down() {
            self.talk_msg = None;
        }

        let ui = self.ui.ui();
        if let Some(talk_msg) = &self.talk_msg {
            ui.window("talk_msg")
                .position([200., 200.], imgui::Condition::Always)
                .size([800., 800.], imgui::Condition::Always)
                .movable(false)
                .resizable(false)
                .collapsible(false)
                .title_bar(false)
                .draw_background(false)
                .build(|| {
                    ui.text(talk_msg.text.as_str());
                });
        }
    }

    fn update_audio(&mut self) {
        for sound in self.sound_sources.values() {
            let mut sound = sound.borrow_mut();
            sound.update();
        }

        self.sound_sources.retain(|_, s| {
            let sound = s.borrow();
            sound.state() != AudioSourceState::Stopped
        });

        self.bgm_source.update();
    }

    fn update_story_pic(&mut self) {
        if let Some(sprite) = &self.story_pic {
            let (start, size) = calc_43_box(&self.ui.ui());

            let style = self
                .ui
                .ui()
                .push_style_var(imgui::StyleVar::WindowPadding([0., 0.]));

            self.ui
                .ui()
                .window("story_pic")
                .position(start, imgui::Condition::Always)
                .size(size, imgui::Condition::Always)
                .movable(false)
                .resizable(false)
                .collapsible(false)
                .title_bar(false)
                .draw_background(false)
                .scroll_bar(false)
                .nav_focus(false)
                .focused(false)
                .mouse_inputs(false)
                .build(|| {
                    Image::new(sprite.imgui_texture_id(), size).build(self.ui.ui());
                });

            style.pop();
        }
    }

    fn update_video(&mut self) {
        if self.video_player.get_state() == radiance::video::VideoStreamState::Playing {
            if self
                .input_engine
                .borrow()
                .get_key_state(Key::Escape)
                .pressed()
            {
                self.video_player.stop();
                return;
            }

            let source_size = self.video_player.get_source_size().unwrap();
            self.movie_texture = crate::utils::play_movie(
                self.ui.ui(),
                &mut self.video_player,
                self.movie_texture,
                source_size,
                false,
            );
        }
    }

    fn anykey_down(&mut self) -> bool {
        self.input_engine
            .borrow()
            .get_key_state(Key::Space)
            .pressed()
            || self
                .input_engine
                .borrow()
                .get_key_state(Key::Escape)
                .pressed()
            || self
                .input_engine
                .borrow()
                .get_key_state(Key::GamePadSouth)
                .pressed()
    }

    fn isfon(&mut self, _f: f64) -> i32 {
        0
    }

    fn fon(&mut self, _f: f64) {}

    fn foff(&mut self, _f: f64) {}

    fn lock_player(&mut self, _f: f64) {}

    fn dark(&mut self, speed: f64) {
        self.actdrop
            .set_darkness(InterpValue::new(0., 1., 0.1 * speed as f32));
    }

    fn undark(&mut self, speed: f64) {
        self.actdrop
            .set_darkness(InterpValue::new(1., 0., 0.1 * speed as f32));
    }

    fn chang_map(&mut self, map_id: f64, _x: f64, _y: f64, _z: f64) {
        let map_id = map_id as i32;
        let scene = Swd5Scene::load(&self.asset_loader, map_id);
        match scene {
            Ok(scene) => {
                self.scene_manager.pop_scene();
                self.scene_manager.push_scene(scene.scene.clone());

                self.scene = Some(scene);
                self.current_map_id = map_id;
            }
            Err(e) => log::error!("chang_map {}: {:?}", map_id, e),
        }
    }

    fn wait_camera(&mut self) {}

    fn camera_mode(&mut self, _mode: f64) {}

    fn story_music_off(&mut self, _f1: f64, _f2: f64) {
        self.bgm_source.stop();
    }

    fn story_music(&mut self, music_id: f64, _f2: f64, _f3: f64, _f4: f64, _f5: f64, _f6: f64) {
        let data = self.asset_loader.load_music(music_id as i32);
        match data {
            Ok(data) => {
                self.bgm_source.set_data(data, Codec::Mp3);
                self.bgm_source.play(true);
            }
            Err(_) => return,
        }
    }

    fn chang_role_map(&mut self, _map_id: f64, _f2: f64, _f3: f64, _f4: f64) {}

    fn set_motion(&mut self, _f1: f64, _f2: f64) {}

    fn set_walks(&mut self, _f1: f64, _f2: f64) {}

    fn play_sound(&mut self, sound_id: f64, _volume: f64) {
        let sound_id = sound_id as i32;
        let data = self.asset_loader.load_sound(sound_id);
        match data {
            Ok(data) => {
                let mut source = self.audio_engine.create_source();
                source.set_data(data, Codec::Mp3);
                source.play(false);

                self.sound_sources.insert(sound_id, RefCell::new(source));
            }
            Err(_) => return,
        }
    }

    fn stop_sound(&mut self, sound_id: f64) {
        let sound_id = sound_id as i32;
        self.sound_sources
            .remove(&sound_id)
            .map(|source| source.borrow_mut().stop());
    }

    fn storymsg(&mut self, text: *const c_char) {
        let text = decode_big5(text);
        let [width, height] = self.ui.ui().io().display_size;

        self.story_msg = Some(StoryMsg {
            text,
            position: [width / 2. - 300., height / 2. - 200.],
        });
    }

    fn talkmsg(&mut self, name: *const c_char, text: *const c_char) {
        let name = decode_big5(name);
        let text = decode_big5(text);

        self.talk_msg = Some(TalkMsg { name, text });
    }

    fn storymsgpos(&mut self, text: *const c_char, x: f64, y: f64) {
        let text = decode_big5(text);
        let (start, size) = calc_43_box(self.ui.ui());
        let x = x as f32 / 960. * size[0];
        let y = y as f32 / 720. * size[1];

        self.story_msg = Some(StoryMsg {
            text,
            position: [x + start[0], y + start[1]],
        });
    }

    fn openstorypic(&mut self, pic_id: f64) {
        let data = self.asset_loader.load_story_pic(pic_id as i32);
        match data {
            Ok(sprite) => {
                self.story_pic = Some(sprite);
            }
            Err(e) => log::error!("openstorypic: {:?}", e),
        }
    }

    fn closestorypic(&mut self) {
        self.story_pic = None;
    }

    fn set_camera_src_pos(&mut self, x: f64, y: f64, z: f64) {
        let scene = self.scene.as_mut().unwrap();
        scene.set_camera_lookat(x as f32, y as f32, z as f32);
    }

    fn set_camera_pos(&mut self, x: f64, y: f64, z: f64) {
        println!("set_camera_pos({}, {}, {})", x, y, z);
    }

    fn chang_camera_view(&mut self, dx: f64, dy: f64, dis: f64, _time: f64) {
        let scene = self.scene.as_mut().unwrap();
        scene.set_camera_delta(dx as f32, dy as f32, dis as f32);
    }

    fn set_role_face_motion(&mut self, _role: f64, _face_motion: f64) {}

    fn play_movie(&mut self, id: f64) {
        let reader = self.asset_loader.load_movie_data(id as u32);
        match reader {
            Ok(reader) => {
                self.video_player.play(
                    self.component_factory.clone(),
                    self.audio_engine.clone(),
                    reader,
                    radiance::video::Codec::Bik,
                    false,
                );
            }
            Err(e) => log::error!("play_movie: {:?}", e),
        }
    }

    fn is_play_movie(&mut self) -> f64 {
        (self.video_player.get_state() == radiance::video::VideoStreamState::Playing) as u32 as f64
    }

    fn anykey(&mut self) -> i32 {
        (self.anykey_down || self.anykey_down()) as i32
    }
}

macro_rules! def_func {
    ($vm: ident, $fn_name: ident $(, [$state: ident])? $(, $param_names: ident : $param_types: ident)* $(-> $ret_type: ident)?) => {
        paste::paste! {
            extern "C" fn $fn_name(state: *mut lua_State) -> i32 {
                unsafe {
                    let v = lua50_32_sys::lua_touserdata(state, lua50_32_sys::LUA_GLOBALSINDEX - 1);

                    let context = &*(v as *const RefCell<SWD5Context>);
                    $(let $state = state;)?
                    $(let $param_names = lua50_32_sys::[<lua_to $param_types>](state, 1);lua50_32_sys::lua_remove(state, 1);)*

                    let mut context = context.borrow_mut();
                    let _ret = context.$fn_name(
                        $($state,)?
                        $($param_names),*
                    );

                    $(lua50_32_sys::[<lua_push $ret_type>](state, _ret.into());)?

                    let _log_str = format!(concat!("{}(", $(concat!("{", stringify!($param_names), ":?},"), )* ")"),
                        stringify!($fn_name),
                        $($param_names=$param_names),*);

                    $(stringify!($ret_type); let _log_str = format!("{} -> {}", _log_str, _ret);)?

                    if stringify!($fn_name) != "anykey" {
                        log::warn!("{}", _log_str);
                    }
                }

                let _ret = 0;
                $(stringify!($ret_type); let _ret = 1;)?

                _ret
            }

            $vm.register(stringify!($fn_name), Some($fn_name));
        }
    };
}

pub fn create_lua_vm(
    asset_loader: &Rc<AssetLoader>,
    context: Rc<RefCell<SWD5Context>>,
) -> anyhow::Result<Lua5032Vm<SWD5Context>> {
    let script = asset_loader.load_main_script()?;
    let vm = Lua5032Vm::new(script, "initiatelua", context)?;

    def_func!(vm, isfon, f: number -> number);
    def_func!(vm, fon, f: number);
    def_func!(vm, foff, f: number);
    def_func!(vm, lock_player, f: number);
    def_func!(vm, dark, speed: number);
    def_func!(vm, undark, speed: number);
    vm.register("sleep", Some(sleep));
    def_func!(vm, chang_map, map: number, x: number, y: number, z: number);
    def_func!(vm, wait_camera);
    def_func!(vm, camera_mode, f: number);
    def_func!(vm, story_music_off, f1: number, f2: number);
    def_func!(vm, story_music, music_id: number, f2: number, f3: number, f4: number, f5: number, f6: number);
    def_func!(vm, chang_role_map, map_id: number, f2: number, f3: number, f4: number);
    def_func!(vm, set_motion, f1: number, f2: number);
    def_func!(vm, set_walks, f1: number, f2: number);
    def_func!(vm, play_sound, sound_id: number, volume: number);
    def_func!(vm, storymsg, text: string);
    def_func!(vm, storymsgpos, text: string, x: number, y: number);
    def_func!(vm, talkmsg, name: string, text: string);
    def_func!(vm, anykey -> number);
    def_func!(vm, openstorypic, pic_id: number);
    def_func!(vm, stop_sound, sound_id: number);
    def_func!(vm, closestorypic);
    def_func!(vm, play_movie, id: number);
    def_func!(vm, is_play_movie -> number);
    def_func!(vm, set_camera_src_pos, x: number, y: number, z: number);
    def_func!(vm, set_camera_pos, x: number, y: number, z: number);
    def_func!(vm, chang_camera_view, dx: number, dy: number, dis: number, time: number);
    def_func!(vm, set_role_face_motion, role: number, face_motion: number);

    Ok(vm)
}

/// Names registered into the Lua global table by [`create_lua_vm`],
/// plus the Lua 5.0 standard-library entries opened by
/// [`Lua5032Vm::new`]. `/v1/script/globals` filters these out so the
/// response carries only the game's own plot state.
///
/// Keep in sync with the `def_func!` / `vm.register` calls above.
pub const RESERVED_GLOBAL_NAMES: &[&str] = &[
    // Host functions registered by `create_lua_vm`.
    "isfon",
    "fon",
    "foff",
    "lock_player",
    "dark",
    "undark",
    "sleep",
    "chang_map",
    "wait_camera",
    "camera_mode",
    "story_music_off",
    "story_music",
    "chang_role_map",
    "set_motion",
    "set_walks",
    "play_sound",
    "storymsg",
    "storymsgpos",
    "talkmsg",
    "anykey",
    "openstorypic",
    "stop_sound",
    "closestorypic",
    "play_movie",
    "is_play_movie",
    "set_camera_src_pos",
    "set_camera_pos",
    "chang_camera_view",
    "set_role_face_motion",
    // Lua 5.0 stdlib (base / table / io / string / math / debug / loadlib).
    "_G",
    "_LOADED",
    "_REQUIREDNAME",
    "_TRACEBACK",
    "_VERSION",
    "LUA_PATH",
    "assert",
    "collectgarbage",
    "coroutine",
    "debug",
    "dofile",
    "error",
    "gcinfo",
    "getfenv",
    "getmetatable",
    "io",
    "ipairs",
    "loadfile",
    "loadlib",
    "loadstring",
    "math",
    "newproxy",
    "next",
    "os",
    "pairs",
    "pcall",
    "print",
    "rawequal",
    "rawget",
    "rawset",
    "require",
    "select",
    "setfenv",
    "setmetatable",
    "string",
    "table",
    "tonumber",
    "tostring",
    "type",
    "unpack",
    "xpcall",
];

extern "C" fn sleep(state: *mut lua_State) -> i32 {
    unsafe {
        let delay = lua50_32_sys::lua_tonumber(state, 1);
        lua50_32_sys::lua_remove(state, 1);
        lua50_32_sys::lua_pushnumber(state, delay);
        lua50_32_sys::lua_yield(state, 1)
    }
}

/// Rejection message for a `/v1/script/eval` call naming a function
/// outside [`SWD5Context::EVAL_ALLOW_LIST`].
pub(crate) fn eval_rejection_message(function: &str) -> String {
    format!(
        "'{function}' is not in the SWD5 script_eval allow-list ({})",
        SWD5Context::EVAL_ALLOW_LIST.join(", ")
    )
}

fn decode_big5(s: *const c_char) -> String {
    let str = unsafe { std::ffi::CStr::from_ptr(s) };
    let str = encoding::all::BIG5_2003.decode(str.to_bytes(), DecoderTrap::Ignore);
    match str {
        Ok(str) => str,
        Err(str) => format!("{:?}", str),
    }
}

struct StoryMsg {
    text: String,
    position: [f32; 2],
}

struct TalkMsg {
    /// Speaker name parsed from the script. Not drawn in the message
    /// box yet, but surfaced to the agent snapshot as `dialog.avatar`.
    name: String,
    text: String,
}

fn calc_43_box(ui: &imgui::Ui) -> ([f32; 2], [f32; 2]) {
    let [width, height] = ui.io().display_size;

    let start = if width > height {
        let x = (width - height * 4. / 3.) / 2.;
        [x, 0.]
    } else {
        let y = (height - width * 3. / 4.) / 2.;
        [0., y]
    };

    let size = if width > height {
        [height * 4. / 3., height]
    } else {
        [width, width * 3. / 4.]
    };

    (start, size)
}
