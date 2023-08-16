use std::rc::Rc;

use dashmap::DashMap;
use imgui::TextureId;

use crate::{rendering::ComponentFactory, utils::SeekRead};

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
}

type DecoderConstructor = fn(Rc<dyn ComponentFactory>) -> Box<dyn VideoStream>;

lazy_static::lazy_static! {
    pub static ref VIDEO_DECODER_MAP: DashMap<Codec, DecoderConstructor> = DashMap::new();
}

pub fn register_video_decoder(codec: Codec, constructor: DecoderConstructor) {
    VIDEO_DECODER_MAP.entry(codec).or_insert(constructor);
}

pub(crate) fn create_stream(
    factory: Rc<dyn ComponentFactory>,
    reader: Box<dyn SeekRead>,
    codec: Codec,
) -> Option<Box<dyn VideoStream>> {
    let entry = VIDEO_DECODER_MAP.get(&codec)?;
    let mut stream = entry.value()(factory);
    stream.set_reader(reader);
    Some(stream)
}
