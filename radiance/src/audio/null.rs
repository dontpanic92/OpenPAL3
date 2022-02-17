use alloc::{boxed::Box, vec::Vec};

use super::{AudioEngine, AudioSource, AudioSourceState};

pub struct NullAudioEngine {}

impl AudioEngine for NullAudioEngine {
    fn create_source(&self) -> Box<dyn AudioSource> {
        Box::new(NullAudioSource {})
    }
}

impl NullAudioEngine {
    pub fn new() -> Self {
        NullAudioEngine {}
    }
}

pub struct NullAudioSource {}

impl AudioSource for NullAudioSource {
    fn update(&mut self) {}

    fn play(&mut self, data: Vec<u8>, codec: super::Codec, looping: bool) {}

    fn restart(&mut self) {}

    fn pause(&mut self) {}

    fn resume(&mut self) {}

    fn stop(&mut self) {}

    fn state(&self) -> AudioSourceState {
        AudioSourceState::Stopped
    }
}
