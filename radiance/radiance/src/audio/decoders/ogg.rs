extern crate lewton;

use super::Decoder;
use lewton::inside_ogg::OggStreamReader;
use std::io::Cursor;

pub struct OggDecoder {
    data: Vec<u8>,
    decoder: OggStreamReader<Cursor<Vec<u8>>>,
}

impl Decoder for OggDecoder {
    fn fetch_samples(&mut self) -> Result<Option<super::Samples>, Box<dyn std::error::Error>> {
        Ok(self.decoder.read_dec_packet_itl().and_then(|s| {
            Ok(s.and_then(|samples| {
                Some(super::Samples {
                    data: samples,
                    sample_rate: self.decoder.ident_hdr.audio_sample_rate as i32,
                    channels: self.decoder.ident_hdr.audio_channels as usize,
                })
            }))
        })?)
    }

    fn reset(&mut self) {
        let cursor = Cursor::new(self.data.clone());
        self.decoder = OggStreamReader::new(cursor).unwrap();
    }
}

impl OggDecoder {
    pub fn new(data: Vec<u8>) -> Self {
        let cursor = Cursor::new(data.clone());
        let decoder = OggStreamReader::new(cursor).unwrap();

        Self { data, decoder }
    }
}
