use std::io::Cursor;

use symphonia::core::{
    audio::SampleBuffer,
    codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL},
    errors::Error,
    formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

use super::Samples;

pub struct SymphoniaDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
}

impl super::Decoder for SymphoniaDecoder {
    fn fetch_samples(&mut self) -> anyhow::Result<Option<Samples>> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(Error::ResetRequired) => {
                    self.reset();
                    return Err(Error::ResetRequired)?;
                }
                Err(Error::IoError(err)) => {
                    if err.kind() == std::io::ErrorKind::UnexpectedEof {
                        return Ok(None);
                    } else {
                        return Err(err)?;
                    }
                }
                Err(err) => {
                    return Err(err)?;
                }
            };

            while !self.format.metadata().is_latest() {
                self.format.metadata().pop();
            }

            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let duration = decoded.capacity() as u64;
                    let mut sample_buf = SampleBuffer::<i16>::new(duration, spec);
                    sample_buf.copy_interleaved_ref(decoded);

                    return Ok(Some(Samples {
                        data: sample_buf.samples().to_vec(),
                        sample_rate: spec.rate as i32,
                        channels: spec.channels.count(),
                    }));
                }
                Err(Error::IoError(_)) => {
                    continue;
                }
                Err(Error::DecodeError(_)) => {
                    continue;
                }
                Err(err) => {
                    Err(err)?;
                }
            }
        }
    }

    fn reset(&mut self) {
        let _ = self.format.seek(
            SeekMode::Accurate,
            SeekTo::TimeStamp {
                ts: 0,
                track_id: self.track_id,
            },
        );
        self.decoder.reset();
    }
}

impl SymphoniaDecoder {
    pub fn new(data: Vec<u8>) -> Self {
        let mss = MediaSourceStream::new(Box::new(Cursor::new(data)), Default::default());
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        let probed = symphonia::default::get_probe()
            .format(
                &Hint::new().with_extension("mp3"),
                mss,
                &fmt_opts,
                &meta_opts,
            )
            .expect("unsupported format");
        let format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .expect("no supported audio tracks");

        let dec_opts: DecoderOptions = Default::default();

        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &dec_opts)
            .expect("unsupported codec");

        let track_id = track.id;
        Self {
            format,
            decoder,
            track_id,
        }
    }
}
