use std::{
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
};

use crate::openpal3::{
    asset_manager::AssetManager,
    audio_mix::{AudioChannel, Pal3AudioMix, mixed_source},
};

use super::persistent_state::PersistentState;
use common::store_ext::StoreExt2;
use radiance::{
    audio::{AudioEngine, AudioMemorySource, AudioSource, AudioSourceState, Codec as AudioCodec},
    rendering::VideoPlayer,
    video::Codec as VideoCodec,
};
use regex::Regex;

pub struct GlobalState {
    audio_engine: Rc<dyn AudioEngine>,
    persistent_state: Rc<RefCell<PersistentState>>,
    fop_state: FopState,
    adv_input_enabled: bool,
    role_controlled: i32,

    asset_mgr: Rc<AssetManager>,
    bgm_source: Box<dyn AudioMemorySource>,
    sound_sources: Vec<Rc<RefCell<Box<dyn AudioMemorySource>>>>,
    default_scene_bgm: HashMap<String, String>,
    video_player: Box<VideoPlayer>,
    audio_mix: Arc<Pal3AudioMix>,

    pass_through_wall: bool,
}

impl GlobalState {
    pub fn new(
        asset_mgr: Rc<AssetManager>,
        audio_engine: Rc<dyn AudioEngine>,
        persistent_state: Rc<RefCell<PersistentState>>,
    ) -> Self {
        let audio_mix = Arc::new(Pal3AudioMix::load());
        let bgm_source = mixed_source(
            audio_engine.create_source(),
            audio_mix.clone(),
            AudioChannel::Music,
        );
        let video_player = asset_mgr.component_factory().create_video_player();
        let sound_sources = vec![];
        let music_path = "/basedata/basedata/datascript/music.txt";
        let default_scene_bgm =
            parse_music_mapping(asset_mgr.vfs().read_to_end_from_gbk(music_path).unwrap());

        let role_controlled = if asset_mgr.game() == crate::GameType::PAL3 {
            persistent_state.borrow().role_controlled()
        } else {
            0
        };
        Self {
            persistent_state,
            audio_engine,
            fop_state: FopState::new(),
            adv_input_enabled: true,
            role_controlled,
            asset_mgr,
            bgm_source,
            sound_sources,
            default_scene_bgm,
            video_player,
            audio_mix,
            pass_through_wall: false,
        }
    }

    pub fn adv_input_enabled(&self) -> bool {
        self.adv_input_enabled
    }

    pub fn set_adv_input_enabled(&mut self, adv_input_enabled: bool) {
        self.adv_input_enabled = adv_input_enabled
    }

    pub fn role_controlled(&self) -> i32 {
        self.role_controlled
    }

    pub fn set_role_controlled(&mut self, role_controlled: i32) {
        self.role_controlled = role_controlled;
        if self.asset_mgr.game() == crate::GameType::PAL3 {
            self.persistent_state
                .borrow_mut()
                .set_role_controlled(role_controlled);
        }
    }

    pub fn play_bgm(&mut self, name: &str) {
        let data = self.asset_mgr.load_music_data(name);
        self.bgm_source.set_data(data, AudioCodec::Mp3);
        self.bgm_source.play(true);
    }

    pub fn play_default_bgm(&mut self) {
        if self.bgm_source.state() != AudioSourceState::Stopped {
            return;
        }

        let (scene_name, sub_scene_name) = {
            let p_state = self.persistent_state.borrow();
            (
                p_state.scene_name().unwrap().to_lowercase(),
                p_state.sub_scene_name().unwrap().to_lowercase(),
            )
        };

        if let Some(name) = self
            .default_scene_bgm
            .get(&format!("{}_{}", scene_name, sub_scene_name))
            .or(self.default_scene_bgm.get(&scene_name))
        {
            let name = name.to_string();
            if name != "NONE" {
                self.play_bgm(&name);
            } else {
                self.bgm_source.stop();
            }
        }
    }

    pub fn bgm_source(&mut self) -> &mut dyn AudioSource {
        self.bgm_source.as_mut()
    }

    pub fn video_player(&mut self) -> &mut VideoPlayer {
        self.video_player.as_mut()
    }

    pub fn play_movie(&mut self, name: &str) -> Option<(u32, u32)> {
        let reader = self.asset_mgr.load_movie_data(name);
        let factory = self.asset_mgr.component_factory();
        self.video_player.play(
            factory,
            self.audio_engine.clone(),
            reader,
            VideoCodec::Bik,
            false,
        )
    }

    pub fn asset_mgr(&self) -> Rc<AssetManager> {
        self.asset_mgr.clone()
    }

    pub fn persistent_state(&self) -> Ref<'_, PersistentState> {
        self.persistent_state.borrow()
    }

    pub fn menu_state(&self) -> crosscom::ComRc<crate::openpal3::comdef::IPal3MenuState> {
        crate::openpal3::menu_state::Pal3MenuState::create_with_persisted_audio(
            self.persistent_state.clone(),
            (self.asset_mgr.game() == crate::GameType::PAL3).then(|| self.asset_mgr.stats_data()),
            self.audio_mix.clone(),
            self.asset_mgr.scene_names(),
        )
    }

    pub fn persistent_state_mut(&mut self) -> RefMut<'_, PersistentState> {
        self.persistent_state.borrow_mut()
    }

    pub fn add_sound_source(&mut self, source: Rc<RefCell<Box<dyn AudioMemorySource>>>) {
        self.sound_sources.push(source);
    }

    pub fn create_sound_source(&self) -> Box<dyn AudioMemorySource> {
        mixed_source(
            self.audio_engine.create_source(),
            self.audio_mix.clone(),
            AudioChannel::Sound,
        )
    }

    pub fn remove_sound_source(&mut self, source: Rc<RefCell<Box<dyn AudioMemorySource>>>) {
        self.sound_sources
            .iter()
            .position(|s| Rc::ptr_eq(s, &source))
            .map(|p| self.sound_sources.remove(p));
    }

    pub fn fop_state_mut(&mut self) -> &mut FopState {
        &mut self.fop_state
    }

    pub fn update(&mut self, _delta_sec: f32) {
        if self.bgm_source.state() == AudioSourceState::Playing {
            self.bgm_source.update();
        }

        self.remove_stopped_sound_sources();
        for source in &mut self.sound_sources {
            if source.borrow().state() == AudioSourceState::Playing {
                source.borrow_mut().update();
            }
        }
    }

    fn remove_stopped_sound_sources(&mut self) {
        self.sound_sources
            .retain(|s| s.borrow().state() != AudioSourceState::Stopped);
    }

    pub fn pass_through_wall(&self) -> bool {
        self.pass_through_wall
    }

    pub fn pass_through_wall_mut(&mut self) -> &mut bool {
        &mut self.pass_through_wall
    }
}

pub enum Fop {
    And,
    Or,
}

pub struct FopState {
    lhs: Option<bool>,
    op: Option<Fop>,
}

impl FopState {
    pub fn new() -> Self {
        Self {
            lhs: None,
            op: None,
        }
    }

    pub fn push_value(&mut self, value: bool) {
        self.lhs = match (&self.lhs, &self.op) {
            (Some(lhs), Some(Fop::And)) => Some(*lhs && value),
            (Some(lhs), Some(Fop::Or)) => Some(*lhs || value),
            (_, _) => Some(value),
        }
    }

    pub fn set_op(&mut self, op: Fop) {
        self.op = Some(op);
    }

    pub fn reset(&mut self) {
        self.lhs = None;
        self.op = None;
    }

    pub fn value(&self) -> Option<bool> {
        self.lhs
    }
}

fn parse_music_mapping(mapping: String) -> HashMap<String, String> {
    let mut scene_mapping = HashMap::new();
    let scene_begin_regex = Regex::new(r"BEGIN,\s*scene").unwrap();
    let scene_end_regex = Regex::new(r"END").unwrap();
    let music_mapping_regex = Regex::new(r"(.+?)\s*\$(.+?)&").unwrap();
    let mut scene_mapping_state = 0;
    for line in mapping.split('\n') {
        match scene_mapping_state {
            0 => {
                if scene_begin_regex.is_match(line) {
                    scene_mapping_state = 1;
                }
            }
            1 => {
                if scene_end_regex.is_match(line) {
                    scene_mapping_state = 2;
                } else {
                    let capture = music_mapping_regex.captures_iter(line).next();
                    if let Some(c) = capture {
                        scene_mapping.insert(c[1].to_lowercase(), c[2].to_string());
                    }
                }
            }
            _ => {}
        }
    }

    scene_mapping
}
