use super::{
    decoders::{Decoder, Mp3Decoder, OggDecoder, Samples, WavDecoder},
    Codec,
};
use super::{AudioEngine, AudioSource, AudioSourceState};
use alto::{Alto, Context, Mono, OutputDevice, Source, Stereo};
use std::rc::Rc;

pub struct OpenAlAudioEngine {
    alto: Alto,
    device: OutputDevice,
    context: Rc<Context>,
}

impl AudioEngine for OpenAlAudioEngine {
    fn create_source(&self) -> Box<dyn AudioSource> {
        Box::new(OpenAlAudioSource::new(self.context.clone()))
    }
}

impl OpenAlAudioEngine {
    pub fn new() -> Self {
        let alto = Alto::load_default().unwrap();
        let device = alto.open(None).unwrap();
        let context = Rc::new(device.new_context(None).unwrap());

        Self {
            alto,
            device,
            context,
        }
    }
}

pub struct OpenAlAudioSource {
    context: Rc<Context>,
    streaming_source: alto::StreamingSource,
    decoder: Option<Box<dyn Decoder>>,
    state: AudioSourceState,
    looping: bool,
    data: Option<Vec<u8>>,
    codec: Option<Codec>,
}

impl AudioSource for OpenAlAudioSource {
    fn update(&mut self) {
        if self.decoder.is_none() {
            return;
        }

        if self.streaming_source.buffers_queued() == 0 {
            self.state = AudioSourceState::Stopped;
        }

        if self.state == AudioSourceState::Stopped || self.state == AudioSourceState::Paused {
            return;
        }

        let mut processed = self.streaming_source.buffers_processed();
        while processed > 0 {
            let frame = self.decoder.as_mut().unwrap().fetch_samples();
            if let Ok(None) = frame {
                if self.looping == true {
                    self.decoder.as_mut().unwrap().reset();
                    continue;
                }
            }

            if let Ok(mut buffer) = self.streaming_source.unqueue_buffer() {
                match frame {
                    Ok(Some(samples)) => {
                        match samples.channels {
                            1 => buffer
                                .set_data::<Mono<i16>, _>(samples.data, samples.sample_rate)
                                .unwrap(),
                            2 => buffer
                                .set_data::<Stereo<i16>, _>(samples.data, samples.sample_rate)
                                .unwrap(),
                            _ => {}
                        }

                        self.streaming_source.queue_buffer(buffer).unwrap();
                    }
                    Ok(None) => {}
                    Err(e) => println!("Error: {}", e),
                }
            }

            processed -= 1;
        }

        // The state changes when the buffers are exhausted
        if self.streaming_source.state() == alto::SourceState::Stopped {
            self.streaming_source.play();
        }
    }

    fn play(&mut self, data: Vec<u8>, codec: Codec, looping: bool) {
        self.stop();

        self.data = Some(data.clone());
        self.codec = Some(codec);

        self.decoder = Some(create_decoder(data, codec));
        self.looping = looping;
        self.play_internal();
    }

    fn restart(&mut self) {
        if self.decoder.is_none() {
            return;
        }

        self.stop();
        self.decoder.as_mut().unwrap().reset();
        self.play_internal();
    }

    fn stop(&mut self) {
        self.state = AudioSourceState::Stopped;
        self.streaming_source.stop();
        while self.streaming_source.unqueue_buffer().is_ok() {}
    }

    fn state(&self) -> AudioSourceState {
        self.state
    }

    fn pause(&mut self) {
        self.state = AudioSourceState::Paused;
        self.streaming_source.pause();
    }

    fn resume(&mut self) {
        if self.state == AudioSourceState::Paused {
            self.state = AudioSourceState::Playing;
            self.streaming_source.play();
        }
    }
}

impl OpenAlAudioSource {
    pub fn new(context: Rc<Context>) -> Self {
        let streaming_source = context.new_streaming_source().unwrap();

        Self {
            context,
            streaming_source,
            decoder: None,
            state: AudioSourceState::Stopped,
            looping: false,
            data: None,
            codec: None,
        }
    }

    fn play_internal(&mut self) {
        for _ in 0..20 {
            let frame = self.decoder.as_mut().unwrap().fetch_samples();
            match frame {
                Ok(Some(samples)) => {
                    let buffer = create_buffer_from_samples(samples, self.context.as_ref());
                    if buffer.is_none() {
                        continue;
                    }

                    self.streaming_source.queue_buffer(buffer.unwrap()).unwrap();
                }
                _ => break,
            }
        }

        self.streaming_source.play();
        self.state = AudioSourceState::Playing;
    }
}

fn create_buffer_from_samples(samples: Samples, context: &Context) -> Option<alto::Buffer> {
    match samples.channels {
        1 => Some(
            context
                .new_buffer::<Mono<i16>, _>(samples.data, samples.sample_rate)
                .unwrap(),
        ),
        2 => Some(
            context
                .new_buffer::<Stereo<i16>, _>(samples.data, samples.sample_rate)
                .unwrap(),
        ),
        _ => None,
    }
}

fn create_decoder(data: Vec<u8>, codec: Codec) -> Box<dyn Decoder> {
    match codec {
        Codec::Mp3 => Box::new(Mp3Decoder::new(data)),
        Codec::Ogg => Box::new(OggDecoder::new(data)),
        Codec::Wav => Box::new(WavDecoder::new(data)),
    }
}
