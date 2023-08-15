#[cfg(not(vita))]
mod mp3;

mod ogg;
mod symphonia;
mod wav;

#[cfg(not(vita))]
pub use mp3::Mp3Decoder;

pub use ogg::OggDecoder;
pub use symphonia::SymphoniaDecoder;
pub use wav::WavDecoder;

pub trait Decoder {
    fn fetch_samples(&mut self) -> anyhow::Result<Option<Samples>>;
    fn reset(&mut self);
}

pub struct Samples {
    pub data: Vec<i16>,
    pub sample_rate: i32,
    pub channels: usize,
}
