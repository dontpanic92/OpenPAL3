use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

use radiance::audio::{AudioEngine, AudioSource, AudioSourceState, Codec};

use crate::asset_manager::AssetManager;

use super::PersistentState;

pub struct SharedState {
    asset_mgr: Rc<AssetManager>,
    bgm_source: Box<dyn AudioSource>,
    sound_sources: Vec<Rc<RefCell<Box<dyn AudioSource>>>>,
    persistent_state: Rc<RefCell<PersistentState>>,
}

impl SharedState {
    pub fn new(
        asset_mgr: Rc<AssetManager>,
        audio_engine: &Rc<dyn AudioEngine>,
        persistent_state: Rc<RefCell<PersistentState>>,
    ) -> Self {
        let bgm_source = audio_engine.create_source();
        let sound_sources = vec![];

        Self {
            asset_mgr,
            bgm_source,
            sound_sources,
            persistent_state,
        }
    }

    pub fn play_bgm(&mut self, name: &str) {
        let data = self.asset_mgr.load_music_data(name);
        self.bgm_source.play(data, Codec::Mp3, true);

        self.persistent_state
            .borrow_mut()
            .set_bgm_name(name.to_string());
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
