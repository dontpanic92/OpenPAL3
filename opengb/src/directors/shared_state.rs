use super::PersistentState;
use crate::asset_manager::AssetManager;
use crate::utilities::StoreExt2;
use radiance::audio::{AudioEngine, AudioSource, AudioSourceState, Codec};
use regex::Regex;
use std::{
    cell::{RefCell, RefMut},
    collections::HashMap,
    rc::Rc,
};

pub struct SharedState {
    asset_mgr: Rc<AssetManager>,
    bgm_source: Box<dyn AudioSource>,
    sound_sources: Vec<Rc<RefCell<Box<dyn AudioSource>>>>,
    persistent_state: Rc<RefCell<PersistentState>>,
    default_scene_bgm: HashMap<String, String>,
}

impl SharedState {
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
            asset_mgr,
            bgm_source,
            sound_sources,
            persistent_state,
            default_scene_bgm,
        }
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
