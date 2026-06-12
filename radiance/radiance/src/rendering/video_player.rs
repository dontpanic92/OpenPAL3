use std::rc::Rc;

use imgui::TextureId;

use crate::{
    audio::AudioEngine,
    utils::SeekRead,
    video::{Codec, VideoStream, VideoStreamState, create_stream},
};

use super::ComponentFactory;

pub struct VideoPlayer {
    stream: Option<Box<dyn VideoStream>>,
    size: Option<(u32, u32)>,
}

impl VideoPlayer {
    pub fn new() -> Self {
        VideoPlayer {
            stream: None,
            size: None,
        }
    }

    pub fn play(
        &mut self,
        factory: Rc<dyn ComponentFactory>,
        audio_engine: Rc<dyn AudioEngine>,
        reader: Box<dyn SeekRead>,
        codec: Codec,
        looping: bool,
    ) -> Option<(u32, u32)> {
        self.size = create_stream(factory, audio_engine, reader, codec).map(|mut stream| {
            let size = stream.play(looping);
            self.stream = Some(stream);
            size
        });

        self.size
    }

    pub fn pause(&mut self) {
        self.stream.as_mut().unwrap().pause()
    }

    pub fn resume(&mut self) {
        self.stream.as_mut().unwrap().resume()
    }

    pub fn stop(&mut self) {
        self.stream.as_mut().unwrap().stop()
    }

    pub fn get_source_size(&self) -> Option<(u32, u32)> {
        self.size
    }

    pub fn get_texture(&mut self, texture_id: Option<TextureId>) -> Option<TextureId> {
        self.stream.as_mut().and_then(|f| f.get_texture(texture_id))
    }

    pub fn get_state(&self) -> VideoStreamState {
        self.stream
            .as_ref()
            .and_then(|f| Some(f.get_state()))
            .unwrap_or(VideoStreamState::Stopped)
    }

    pub fn duration_ms(&self) -> i64 {
        self.stream.as_ref().map(|s| s.duration_ms()).unwrap_or(0)
    }

    pub fn position_ms(&self) -> i64 {
        self.stream.as_ref().map(|s| s.position_ms()).unwrap_or(0)
    }

    pub fn seek_ms(&mut self, ms: i64) {
        if let Some(s) = self.stream.as_mut() {
            s.seek_ms(ms);
        }
    }

    pub fn set_looping(&mut self, looping: bool) {
        if let Some(s) = self.stream.as_mut() {
            s.set_looping(looping);
        }
    }

    pub fn looping(&self) -> bool {
        self.stream.as_ref().map(|s| s.looping()).unwrap_or(false)
    }

    pub fn restart(&mut self) {
        if let Some(s) = self.stream.as_mut() {
            s.restart();
        }
    }
}
