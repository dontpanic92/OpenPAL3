use minimp3::{Decoder, Error};
use std::io::Cursor;

pub struct Mp3Decoder {
    decoder: Decoder<Cursor<Vec<u8>>>,
}

impl super::Decoder for Mp3Decoder {
    fn fetch_samples(&mut self) -> Result<Option<super::Samples>, Box<dyn std::error::Error>> {
        self.decoder
            .next_frame()
            .and_then(|frame| {
                Ok(Some(super::Samples {
                    data: frame.data,
                    sample_rate: frame.sample_rate,
                    channels: frame.channels,
                }))
            })
            .or_else(|err| match err {
                Error::Eof => Ok(None),
                e => Err(e)?,
            })
    }

    fn reset(&mut self) {
        self.decoder.reader_mut().set_position(0);
    }
}

impl Mp3Decoder {
    pub fn new(data: Vec<u8>) -> Self {
        let cursor = Cursor::new(data);
        Self {
            decoder: Decoder::new(cursor),
        }
    }
}
