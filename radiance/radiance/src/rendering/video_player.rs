use std::rc::Rc;

use imgui::TextureId;

use crate::{
    utils::SeekRead,
    video::{create_stream, Codec, VideoStream, VideoStreamState},
};

use super::ComponentFactory;

pub struct VideoPlayer {
    stream: Option<Box<dyn VideoStream>>,
}

impl VideoPlayer {
    pub fn new() -> Self {
        VideoPlayer { stream: None }
    }

    pub fn play(
        &mut self,
        factory: Rc<dyn ComponentFactory>,
        reader: Box<dyn SeekRead>,
        codec: Codec,
        looping: bool,
    ) -> Option<(u32, u32)> {
        let size: Option<(u32, u32)> = create_stream(factory, reader, codec).map(|mut stream| {
            let size = stream.play(looping);
            self.stream = Some(stream);
            size
        });
        size
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

    pub fn get_texture(&mut self, texture_id: Option<TextureId>) -> Option<TextureId> {
        self.stream.as_mut().unwrap().get_texture(texture_id)
    }

    pub fn get_state(&self) -> VideoStreamState {
        self.stream.as_ref().unwrap().get_state()
    }
}
