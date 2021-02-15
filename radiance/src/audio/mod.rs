mod decoders;
mod openal;

pub use openal::OpenAlAudioEngine;

#[derive(Copy, Clone)]
pub enum Codec {
    Wav,
    Mp3,
    Ogg,
}

pub trait AudioEngine {
    fn create_source(&self) -> Box<dyn AudioSource>;
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum AudioSourceState {
    Stopped,
    Playing,
    Paused,
}

pub trait AudioSource {
    fn update(&mut self);

    fn play(&mut self, data: Vec<u8>, codec: Codec, looping: bool);
    fn restart(&mut self);
    fn pause(&mut self);
    fn resume(&mut self);

    fn stop(&mut self);
    fn state(&self) -> AudioSourceState;
}
