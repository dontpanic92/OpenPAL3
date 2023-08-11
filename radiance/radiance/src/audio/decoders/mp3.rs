use minimp3::{Decoder, Error};
use std::{io::Cursor, rc::Rc};

pub struct Mp3Decoder {
    data: SharedDataBuffer,
    decoder: Decoder<Cursor<SharedDataBuffer>>,
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
        self.decoder = Decoder::new(Cursor::new(self.data.clone()));
    }
}

impl Mp3Decoder {
    pub fn new(data: Vec<u8>) -> Self {
        let data = SharedDataBuffer {
            buffer: Rc::new(data),
        };
        let decoder = Decoder::new(Cursor::new(data.clone()));
        Self { data, decoder }
    }
}

#[derive(Clone)]
struct SharedDataBuffer {
    pub buffer: Rc<Vec<u8>>,
}

impl AsRef<[u8]> for SharedDataBuffer {
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref().as_ref()
    }
}
