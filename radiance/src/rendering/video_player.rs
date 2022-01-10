use std::rc::Rc;

use imgui::TextureId;

#[cfg(feature = "ffmpeg")]
use crate::video::VideoStreamFFmpeg;
use crate::video::{Codec, VideoStream, VideoStreamState};

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
        data: Vec<u8>,
        codec: Codec,
        looping: bool,
    ) -> Option<(u32, u32)> {
        let size: Option<(u32, u32)> = create_stream(factory, data, codec).map(|mut stream| {
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

    pub fn get_texture(&self, texture_id: Option<TextureId>) -> Option<TextureId> {
        self.stream.as_ref().unwrap().get_texture(texture_id)
    }

    pub fn get_state(&self) -> VideoStreamState {
        self.stream.as_ref().unwrap().get_state()
    }
}

fn create_stream(
    factory: Rc<dyn ComponentFactory>,
    data: Vec<u8>,
    codec: Codec,
) -> Option<Box<dyn VideoStream>> {
    {
        #[cfg(feature = "ffmpeg")]
        {
            let mut video_stream = match codec {
                Codec::Bik => Box::new(VideoStreamFFmpeg::new(factory)),
                Codec::Webm => Box::new(VideoStreamFFmpeg::new(factory)),
                Codec::Theora => Box::new(VideoStreamFFmpeg::new(factory)),
            };
            video_stream.set_data(data);
            return Some(video_stream);
        }

        #[cfg(not(feature = "ffmpeg"))]
        None
    }
}
