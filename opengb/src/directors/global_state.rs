use super::PersistentState;
use crate::asset_manager::AssetManager;
use crate::utilities::StoreExt2;
use radiance::audio::{AudioEngine, AudioSource, AudioSourceState, Codec};
use regex::Regex;
use std::{
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    rc::Rc,
};

pub struct GlobalState {
    persistent_state: Rc<RefCell<PersistentState>>,
    fop_state: FopState,
    input_enabled: bool,
    role_controlled: i32,

    asset_mgr: Rc<AssetManager>,
    bgm_source: Box<dyn AudioSource>,
    sound_sources: Vec<Rc<RefCell<Box<dyn AudioSource>>>>,
    default_scene_bgm: HashMap<String, String>,
}

impl GlobalState {
    pub fn new(
        asset_mgr: Rc<AssetManager>,
        audio_engine: &Rc<dyn AudioEngine>,
        persistent_state: Rc<RefCell<PersistentState>>,
    ) -> Self {
        let bgm_source = audio_engine.create_source();
        let sound_sources = vec![];
        let default_scene_bgm = parse_music_mapping(
            asset_mgr
                .vfs()
                .read_to_end_from_gbk("/basedata/basedata\\datascript\\music.txt")
                .unwrap(),
        );

        Self {
            persistent_state,
            fop_state: FopState::new(),
            input_enabled: true,
            role_controlled: 0,
            asset_mgr,
            bgm_source,
            sound_sources,
            default_scene_bgm,
        }
    }

    pub fn input_enabled(&self) -> bool {
        self.input_enabled
    }

    pub fn set_input_enabled(&mut self, input_enabled: bool) {
        self.input_enabled = input_enabled
    }

    pub fn role_controlled(&self) -> i32 {
        self.role_controlled
    }

    pub fn set_role_controlled(&mut self, role_controlled: i32) {
        self.role_controlled = role_controlled
    }

    pub fn play_bgm(&mut self, name: &str) {
        let data = self.asset_mgr.load_music_data(name);
        self.bgm_source.play(data, Codec::Mp3, true);
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

    pub fn persistent_state(&self) -> Ref<PersistentState> {
        self.persistent_state.borrow()
    }

    pub fn persistent_state_mut(&mut self) -> RefMut<PersistentState> {
        self.persistent_state.borrow_mut()
    }

    pub fn add_sound_source(&mut self, source: Rc<RefCell<Box<dyn AudioSource>>>) {
        self.sound_sources.push(source);
    }

    pub fn remove_sound_source(&mut self, source: Rc<RefCell<Box<dyn AudioSource>>>) {
        self.sound_sources
            .iter()
            .position(|s| Rc::ptr_eq(s, &source))
            .map(|p| self.sound_sources.remove(p));
    }

    pub fn fop_state_mut(&mut self) -> &mut FopState {
        &mut self.fop_state
    }

    pub fn update(&mut self, delta_sec: f32) {
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
            .drain_filter(|s| s.borrow().state() == AudioSourceState::Stopped);
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
