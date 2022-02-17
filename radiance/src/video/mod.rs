use alloc::rc::Rc;

use crate::ui::TextureId;
use alloc::{boxed::Box, vec::Vec};

use crate::rendering::ComponentFactory;

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
    fn set_data(&mut self, data: Vec<u8>);

    fn play(&mut self, looping: bool) -> (u32, u32);
    fn stop(&mut self);
    fn pause(&mut self);
    fn resume(&mut self);

    fn get_texture(&mut self, texture_id: Option<TextureId>) -> Option<TextureId>;
    fn get_state(&self) -> VideoStreamState;
}

type DecoderConstructor = fn(Rc<dyn ComponentFactory>) -> Box<dyn VideoStream>;

#[cfg(feature = "std")]
mod internal {
    use std::rc::Rc;

    use crate::rendering::ComponentFactory;

    use super::{Codec, DecoderConstructor, VideoStream};
    use dashmap::DashMap;

    lazy_static::lazy_static! {
        pub static ref VIDEO_DECODER_MAP: DashMap<Codec, DecoderConstructor> = DashMap::new();
    }

    pub fn register_video_decoder(codec: Codec, constructor: DecoderConstructor) {
        VIDEO_DECODER_MAP.entry(codec).or_insert(constructor);
    }

    pub fn get_decoder(
        factory: Rc<dyn ComponentFactory>,
        data: Vec<u8>,
        codec: Codec,
    ) -> Option<Box<dyn VideoStream>> {
        let entry = VIDEO_DECODER_MAP.get(&codec)?;
        let mut stream = entry.value()(factory);
        stream.set_data(data);
        Some(stream)
    }
}

#[cfg(feature = "no_std")]
mod internal {
    use super::{Codec, DecoderConstructor};
    use hashbrown::HashMap;
    use spin::RwLock;

    lazy_static::lazy_static! {
        pub static ref VIDEO_DECODER_MAP: RwLock<HashMap<Codec, DecoderConstructor>> = RwLock::new(HashMap::new());
    }

    pub fn register_video_decoder(codec: Codec, constructor: DecoderConstructor) {
        let mut map = VIDEO_DECODER_MAP.write();
        let _ = map.try_insert(codec, constructor);
    }

    pub fn get_decoder(
        factory: Rc<dyn ComponentFactory>,
        data: Vec<u8>,
        codec: Codec,
    ) -> Option<Box<dyn VideoStream>> {
        let guard = VIDEO_DECODER_MAP.read();
        let entry = guard.get(&codec)?;
        let mut stream = entry(factory);
        stream.set_data(data);
        Some(stream)
    }
}

pub use internal::register_video_decoder;

pub(crate) fn create_stream(
    factory: Rc<dyn ComponentFactory>,
    data: Vec<u8>,
    codec: Codec,
) -> Option<Box<dyn VideoStream>> {
    internal::get_decoder(factory, data, codec)
}
