#[cfg(feature = "ffmpeg")]
mod ffmpeg;

#[cfg(feature = "ffmpeg")]
pub use self::ffmpeg::VideoStreamFFmpeg;

use imgui::TextureId;

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum VideoStreamState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Copy, Clone)]
pub enum Codec {
    Bik,
    Webm,
    Theora,
}

pub trait VideoStream {
    fn set_data(&mut self, data: Vec<u8>);

    fn play(&mut self, looping: bool) -> (u32, u32);
    fn stop(&mut self);
    fn pause(&mut self);
    fn resume(&mut self);

    fn get_texture(&mut self, texture_id: Option<TextureId>) -> Option<TextureId>;
    fn get_state(&self) -> VideoStreamState;
}
