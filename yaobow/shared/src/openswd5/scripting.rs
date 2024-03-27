use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crosscom::ComRc;
use encoding::{DecoderTrap, Encoding};
use imgui::{Image, TextureId};
use lua50_32_sys::lua_State;
use radiance::{
    audio::{AudioEngine, AudioMemorySource, AudioSourceState, Codec},
    comdef::ISceneManager,
    input::{InputEngine, Key},
    math::Vec3,
    radiance::UiManager,
    rendering::{ComponentFactory, Sprite, VideoPlayer},
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

    bgm_source: Box<dyn AudioMemorySource>,
    sound_sources: HashMap<i32, RefCell<Box<dyn AudioMemorySource>>>,
    story_msg: Option<StoryMsg>,
    story_pic: Option<Sprite>,
    talk_msg: Option<TalkMsg>,

    movie_texture: Option<TextureId>,
}

impl SWD5Context {
    pub fn new(
        asset_loader: Rc<AssetLoader>,
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
        component_factory: Rc<dyn ComponentFactory>,
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
            scene_manager: unsafe { ComRc::from_raw_pointer(std::ptr::null()) },
            scene: None,
            bgm_source,
            sound_sources: HashMap::new(),
            story_msg: None,
            story_pic: None,
            talk_msg: None,
            movie_texture: None,
        }
    }

    pub fn set_scene_manager(&mut self, mut scene_manager: ComRc<ISceneManager>) {
        if self.scene_manager.is_null() {
            std::mem::swap(&mut self.scene_manager, &mut scene_manager);
            std::mem::forget(scene_manager);
        } else {
            self.scene_manager = scene_manager;
        }
    }

    pub fn update(&mut self, _delta_sec: f32) {
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

    fn isfon(&mut self, f: f64) -> i32 {
        0
    }

    fn fon(&mut self, f: f64) {}

    fn foff(&mut self, f: f64) {}

    fn lock_player(&mut self, f: f64) {}

    fn dark(&mut self, speed: f64) {}

    fn undark(&mut self, speed: f64) {}

    fn chang_map(&mut self, map_id: f64, x: f64, y: f64, z: f64) {
        let map_id = map_id as i32;
        let scene = Swd5Scene::load(&self.asset_loader, map_id);
        match scene {
            Ok(scene) => {
                self.scene_manager.pop_scene();
                self.scene_manager.push_scene(scene.scene.clone());

                self.scene = Some(scene);
            }
            Err(e) => log::error!("chang_map {}: {:?}", map_id, e),
        }
    }

    fn wait_camera(&mut self) {}

    fn camera_mode(&mut self, mode: f64) {}

    fn story_music_off(&mut self, f1: f64, f2: f64) {
        self.bgm_source.stop();
    }

    fn story_music(&mut self, music_id: f64, f2: f64, f3: f64, f4: f64, f5: f64, f6: f64) {
        let data = self.asset_loader.load_music(music_id as i32);
        match data {
            Ok(data) => {
                self.bgm_source.set_data(data, Codec::Mp3);
                self.bgm_source.play(true);
            }
            Err(_) => return,
        }
    }

    fn chang_role_map(&mut self, map_id: f64, f2: f64, f3: f64, f4: f64) {}

    fn set_motion(&mut self, f1: f64, f2: f64) {}

    fn set_walks(&mut self, f1: f64, f2: f64) {}

    fn play_sound(&mut self, sound_id: f64, volume: f64) {
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

    fn storymsg(&mut self, text: *const i8) {
        let text = decode_big5(text);
        let [width, height] = self.ui.ui().io().display_size;

        self.story_msg = Some(StoryMsg {
            text,
            position: [width / 2. - 300., height / 2. - 200.],
        });
    }

    fn talkmsg(&mut self, name: *const i8, text: *const i8) {
        let name = decode_big5(name);
        let text = decode_big5(text);

        self.talk_msg = Some(TalkMsg { name, text });
    }

    fn storymsgpos(&mut self, text: *const i8, x: f64, y: f64) {
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
        println!("set_camera_src_pos({}, {}, {})", x, y, z);
    }

    fn set_camera_pos(&mut self, x: f64, y: f64, z: f64) {
        println!("set_camera_pos({}, {}, {})", x, y, z);
    }

    fn chang_camera_view(&mut self, dx: f64, dy: f64, dis: f64, time: f64) {
        println!("chang_camera_view({}, {}, {})", dx, dy, dis);
    }

    fn set_role_face_motion(&mut self, role: f64, face_motion: f64) {}

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
        self.anykey_down() as i32
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

extern "C" fn sleep(state: *mut lua_State) -> i32 {
    unsafe {
        let delay = lua50_32_sys::lua_tonumber(state, 1);
        lua50_32_sys::lua_remove(state, 1);
        lua50_32_sys::lua_pushnumber(state, delay);
        lua50_32_sys::lua_yield(state, 1)
    }
}

fn decode_big5(s: *const i8) -> String {
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
