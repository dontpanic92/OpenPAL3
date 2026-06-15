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

    /// Per-frame tick. The engine implementation walks every live
    /// source it has minted and forwards the tick (e.g. unqueues
    /// drained OpenAL streaming buffers, feeds fresh decoded samples,
    /// honours looping at EOF). Default impl is a no-op so stub /
    /// test backends don't need to be aware of it; the production
    /// `OpenAlAudioEngine` overrides it. `CoreRadianceEngine::update`
    /// drives this once per frame.
    fn update(&self, _delta_sec: f32) {}

    /// Update the global listener pose used for 3D spatialization.
    /// `position` is the listener's world position; `forward` and `up`
    /// are its orientation basis vectors (need not be normalized). The
    /// production OpenAL backend forwards this to the AL listener;
    /// stub / test backends leave it as a no-op. `CoreRadianceEngine`
    /// drives this once per frame from the active scene's camera.
    fn set_listener(&self, _position: [f32; 3], _forward: [f32; 3], _up: [f32; 3]) {}
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

    /// 3D spatialization controls. All have default no-op
    /// implementations so non-positional backends (stub / test) and
    /// callers that don't care about spatial audio are unaffected. The
    /// OpenAL backend forwards each to the underlying streaming source.
    ///
    /// `set_relative(true)` makes the source position interpreted
    /// relative to the listener (a position of `[0, 0, 0]` then plays
    /// at the listener with no attenuation/panning) — used for
    /// non-spatial nodes such as BGM/UI.
    fn set_position(&mut self, _position: [f32; 3]) {}
    fn set_gain(&mut self, _gain: f32) {}
    fn set_relative(&mut self, _relative: bool) {}
    fn set_reference_distance(&mut self, _distance: f32) {}
    fn set_rolloff_factor(&mut self, _factor: f32) {}
    fn set_max_distance(&mut self, _distance: f32) {}
}

pub trait AudioMemorySource: AudioSource {
    fn set_data(&mut self, data: Vec<u8>, codec_hint: Codec);
}

pub trait AudioCustomDecoderSource: AudioSource {
    fn set_decoder(&mut self, reader: Box<dyn Decoder>);
}
