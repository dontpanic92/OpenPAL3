use std::rc::Rc;

use dashmap::DashMap;
use imgui::TextureId;

use crate::{audio::AudioEngine, rendering::ComponentFactory, utils::SeekRead};

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum VideoStreamState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub enum Codec {
    Bik,
    Webm,
    Theora,
}

pub trait VideoStream {
    fn set_reader(&mut self, reader: Box<dyn SeekRead>);

    fn play(&mut self, looping: bool) -> (u32, u32);
    fn stop(&mut self);
    fn pause(&mut self);
    fn resume(&mut self);

    fn get_texture(&mut self, texture_id: Option<TextureId>) -> Option<TextureId>;
    fn get_state(&self) -> VideoStreamState;

    /// Total duration in milliseconds, or 0 if unknown / not yet
    /// initialized.
    fn duration_ms(&self) -> i64 {
        0
    }

    /// Current playback position in milliseconds within the current
    /// loop. Returns 0 when not playing.
    fn position_ms(&self) -> i64 {
        0
    }

    /// Seek to `ms` milliseconds into the stream. Implementations may
    /// clamp to `[0, duration_ms]`. No-op for streams that don't
    /// support seeking.
    fn seek_ms(&mut self, _ms: i64) {}

    /// Toggle looping at runtime without re-opening the stream.
    fn set_looping(&mut self, _looping: bool) {}

    /// Whether the stream is configured to loop.
    fn looping(&self) -> bool {
        false
    }

    /// Restart from the beginning. Default implementation seeks to 0
    /// and resumes.
    fn restart(&mut self) {
        self.seek_ms(0);
        self.resume();
    }
}

type DecoderConstructor = fn(Rc<dyn ComponentFactory>, Rc<dyn AudioEngine>) -> Box<dyn VideoStream>;

lazy_static::lazy_static! {
    pub static ref VIDEO_DECODER_MAP: DashMap<Codec, DecoderConstructor> = DashMap::new();
}

pub fn register_video_decoder(codec: Codec, constructor: DecoderConstructor) {
    VIDEO_DECODER_MAP.entry(codec).or_insert(constructor);
}

pub(crate) fn create_stream(
    factory: Rc<dyn ComponentFactory>,
    audio_engine: Rc<dyn AudioEngine>,
    reader: Box<dyn SeekRead>,
    codec: Codec,
) -> Option<Box<dyn VideoStream>> {
    let entry = VIDEO_DECODER_MAP.get(&codec)?;
    let mut stream = entry.value()(factory, audio_engine);
    stream.set_reader(reader);
    Some(stream)
}
