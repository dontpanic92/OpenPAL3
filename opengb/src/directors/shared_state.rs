use std::{cell::RefCell, rc::Rc};

use radiance::audio::{AudioEngine, AudioSource, AudioSourceState};

pub struct SharedState {
    bgm_source: Box<dyn AudioSource>,
    sound_sources: Vec<Rc<RefCell<Box<dyn AudioSource>>>>,
}

impl SharedState {
    pub fn new(audio_engine: &Rc<dyn AudioEngine>) -> Self {
        let bgm_source = audio_engine.create_source();
        let sound_sources = vec![];

        Self {
            bgm_source,
            sound_sources,
        }
    }

    pub fn bgm_source(&mut self) -> &mut dyn AudioSource {
        self.bgm_source.as_mut()
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
