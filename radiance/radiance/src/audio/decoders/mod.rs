mod ogg;
mod symphonia;
mod wav;

pub use self::symphonia::SymphoniaDecoder;
pub use ogg::OggDecoder;
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
