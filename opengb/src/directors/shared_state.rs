use radiance::audio::{AudioEngine, AudioSource, AudioSourceState};

pub struct SharedState {
    bgm_source: Box<dyn AudioSource>,
    sound_source: Box<dyn AudioSource>,
}

impl SharedState {
    pub fn new(audio_engine: &dyn AudioEngine) -> Self {
        let bgm_source = audio_engine.create_source();
        let sound_source = audio_engine.create_source();

        Self {
            bgm_source,
            sound_source,
        }
    }

    pub fn bgm_source(&mut self) -> &mut dyn AudioSource {
        self.bgm_source.as_mut()
    }

    pub fn sound_source(&mut self) -> &mut dyn AudioSource {
        self.sound_source.as_mut()
    }

    pub fn update(&mut self, delta_sec: f32) {
        if self.bgm_source.state() == AudioSourceState::Playing {
            self.bgm_source.update();
        }

        if self.sound_source.state() == AudioSourceState::Playing {
            self.sound_source.update();
        }
    }
}
