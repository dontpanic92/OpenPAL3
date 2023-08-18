mod decoders;
mod openal;

pub use decoders::{Decoder, Samples};
pub use openal::OpenAlAudioEngine;

#[derive(Copy, Clone, PartialEq)]
pub enum Codec {
    Wav,
    Mp3,
    Ogg,
}

pub trait AudioEngine {
    fn create_source(&self) -> Box<dyn AudioMemorySource>;
    fn create_custom_decoder_source(&self) -> Box<dyn AudioCustomDecoderSource>;
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum AudioSourceState {
    Stopped,
    Playing,
    Paused,
}

pub trait AudioSource: Send + Sync {
    fn update(&mut self);

    fn play(&mut self, looping: bool);
    fn restart(&mut self);
    fn pause(&mut self);
    fn resume(&mut self);

    fn stop(&mut self);
    fn state(&self) -> AudioSourceState;
}

pub trait AudioMemorySource: AudioSource {
    fn set_data(&mut self, data: Vec<u8>, codec_hint: Codec);
}

pub trait AudioCustomDecoderSource: AudioSource {
    fn set_decoder(&mut self, reader: Box<dyn Decoder>);
}
