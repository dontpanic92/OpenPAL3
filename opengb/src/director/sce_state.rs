use radiance::audio::{AudioEngine, AudioSource};
use std::any::Any;
use std::collections::HashMap;

pub struct SceState {
    bgm_source: Box<dyn AudioSource>,
    sound_source: Box<dyn AudioSource>,
    run_mode: i32,
    ext: HashMap<String, Box<dyn Any>>,
}

impl SceState {
    pub fn new(audio_engine: &dyn AudioEngine) -> Self {
        let bgm_source = audio_engine.create_source();
        let sound_source = audio_engine.create_source();
        let ext = HashMap::<String, Box<dyn Any>>::new();

        Self {
            bgm_source,
            sound_source,
            run_mode: 1,
            ext,
        }
    }

    pub fn bgm_source(&mut self) -> &mut dyn AudioSource {
        self.bgm_source.as_mut()
    }

    pub fn sound_source(&mut self) -> &mut dyn AudioSource {
        self.sound_source.as_mut()
    }

    pub fn run_mode(&self) -> i32 {
        self.run_mode
    }

    pub fn set_run_mode(&mut self, run_mode: i32) {
        self.run_mode = run_mode;
    }

    pub fn ext_mut(&mut self) -> &mut HashMap<String, Box<dyn Any>> {
        &mut self.ext
    }
}
