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
        Box::new(OpenAlAudioSource::new(&self.context))
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
}

impl AudioSource for OpenAlAudioSource {
    fn update(&mut self) {
        if self.decoder.is_none() {
            return;
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

            let mut buffer = self.streaming_source.unqueue_buffer().unwrap();
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
                },
                Ok(None) => self.state = AudioSourceState::Stopped,
                Err(e) => println!("Error: {}", e),
            }

            processed -= 1;
        }

        // The state changes when the buffers are exhausted
        if self.streaming_source.state() == alto::SourceState::Stopped {
            self.streaming_source.play();
        }
    }

    fn play(&mut self, data: Vec<u8>, codec: Codec, looping: bool) {
        let mut decoder = create_decoder(data, codec);

        while self.streaming_source.unqueue_buffer().is_ok() {}

        for _ in 0..20 {
            let frame = decoder.fetch_samples();
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

        self.decoder = Some(decoder);
        self.looping = looping;
        self.streaming_source.play();
        self.state = AudioSourceState::Playing;
    }

    fn restart(&mut self) {
        if self.decoder.is_none() {
            return;
        }

        self.decoder.as_mut().unwrap().reset();
        self.streaming_source.play();
        self.state = AudioSourceState::Playing;
    }

    fn stop(&mut self) {
        self.state = AudioSourceState::Stopped;
        self.streaming_source.stop();
    }

    fn state(&self) -> AudioSourceState {
        self.state
    }
}

impl OpenAlAudioSource {
    pub fn new(context: &Rc<Context>) -> Self {
        let streaming_source = context.new_streaming_source().unwrap();

        Self {
            context: context.clone(),
            streaming_source,
            decoder: None,
            state: AudioSourceState::Stopped,
            looping: false,
        }
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
