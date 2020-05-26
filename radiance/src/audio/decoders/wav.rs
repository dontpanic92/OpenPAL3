use super::Decoder;
use hound::WavReader;
use std::io::Cursor;
use std::iter::Iterator;

pub struct WavDecoder {
    decoder: WavReader<Cursor<Vec<u8>>>,
}

impl Decoder for WavDecoder {
    fn fetch_samples(&mut self) -> Result<Option<super::Samples>, Box<dyn std::error::Error>> {
        let samples = self
            .decoder
            .samples()
            .take(1024)
            .collect::<Result<Vec<i16>, hound::Error>>()?;
        if samples.len() == 0 {
            Ok(None)
        } else {
            Ok(Some(super::Samples {
                data: samples,
                sample_rate: self.decoder.spec().sample_rate as i32,
                channels: self.decoder.spec().channels as usize,
            }))
        }
    }

    fn reset(&mut self) {
        self.decoder.seek(0).unwrap();
    }
}

impl WavDecoder {
    pub fn new(data: Vec<u8>) -> Self {
        let cursor = Cursor::new(data);
        let decoder = WavReader::new(cursor).unwrap();

        Self { decoder }
    }
}
