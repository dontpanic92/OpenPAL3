mod mp3;
mod ogg;
mod wav;

pub use mp3::Mp3Decoder;
pub use ogg::OggDecoder;
pub use wav::WavDecoder;

pub trait Decoder {
    fn fetch_samples(&mut self) -> Result<Option<Samples>, Box<dyn std::error::Error>>;
    fn reset(&mut self);
}

pub struct Samples {
    pub data: Vec<i16>,
    pub sample_rate: i32,
    pub channels: usize,
}
