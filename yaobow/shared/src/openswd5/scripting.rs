use std::{cell::RefCell, collections::HashMap, rc::Rc};

use encoding::{DecoderTrap, Encoding};
use imgui::Ui;
use lua50_32_sys::lua_State;
use radiance::{
    audio::{AudioEngine, AudioMemorySource, AudioSourceState, Codec},
    input::{InputEngine, Key},
    radiance::UiManager,
};

use crate::scripting::lua50_32::Lua5032Vm;

use super::asset_loader::AssetLoader;

pub struct SWD5Context {
    asset_loader: Rc<AssetLoader>,
    audio_engine: Rc<dyn AudioEngine>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
    ui: Rc<UiManager>,

    bgm_source: Box<dyn AudioMemorySource>,
    sound_sources: HashMap<i32, RefCell<Box<dyn AudioMemorySource>>>,
    story_msg: Option<StoryMsg>,
}

impl SWD5Context {
    pub fn new(
        asset_loader: Rc<AssetLoader>,
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
        ui: Rc<UiManager>,
    ) -> Self {
        let bgm_source = audio_engine.create_source();
        Self {
            asset_loader,
            audio_engine,
            input_engine,
            ui,
            bgm_source,
            sound_sources: HashMap::new(),
            story_msg: None,
        }
    }

    pub fn update(&mut self, _delta_sec: f32) {
        self.update_audio();
        self.update_storymsg();
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

    fn anykey_down(&mut self) -> bool {
        self.input_engine
            .borrow()
            .get_key_state(Key::Space)
            .is_down()
            || self
                .input_engine
                .borrow()
                .get_key_state(Key::Escape)
                .is_down()
            || self
                .input_engine
                .borrow()
                .get_key_state(Key::GamePadSouth)
                .is_down()
    }

    fn isfon(&mut self, f: f64) -> i32 {
        0
    }

    fn fon(&mut self, f: f64) {}

    fn lock_player(&mut self, f: f64) {}

    fn dark(&mut self, speed: f64) {}

    fn chang_map(&mut self, map: f64, x: f64, y: f64, z: f64) {}

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

    fn storymsg(&mut self, text: *const i8) {
        let text = decode_big5(text);
        let [width, height] = self.ui.ui().io().display_size;

        self.story_msg = Some(StoryMsg {
            text,
            position: [width / 2. - 300., height / 2. - 200.],
        });
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

                    // log::warn!("{}", _log_str);
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
    def_func!(vm, lock_player, f: number);
    def_func!(vm, dark, speed: number);
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
    def_func!(vm, anykey -> number);

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
