use super::{
    decoders::{Decoder, OggDecoder, Samples, SymphoniaDecoder, WavDecoder},
    AudioCustomDecoderSource, AudioMemorySource, Codec,
};
use super::{AudioEngine, AudioSource, AudioSourceState};
use alto::{Alto, AltoResult, Context, Mono, Source, Stereo};
use std::sync::Arc;

pub struct OpenAlAudioEngine {
    context: Arc<Context>,
}

impl AudioEngine for OpenAlAudioEngine {
    fn create_source(&self) -> Box<dyn AudioMemorySource> {
        Box::new(OpenAlAudioMemorySource::new(self.context.clone()))
    }

    fn create_custom_decoder_source(&self) -> Box<dyn AudioCustomDecoderSource> {
        Box::new(OpenAlAudioCustomDecoderSource::new(self.context.clone()))
    }
}

impl OpenAlAudioEngine {
    pub fn new() -> Self {
        let alto = Alto::load_default().unwrap();
        let device = alto.open(None).unwrap();
        let context = Arc::new(device.new_context(None).unwrap());

        Self { context }
    }
}

pub struct OpenAlAudioSource<T: Send + Sync> {
    context: Arc<Context>,
    streaming_source: alto::StreamingSource,
    decoder: Option<Box<dyn Decoder>>,
    state: AudioSourceState,
    looping: bool,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Send + Sync> AudioSource for OpenAlAudioSource<T> {
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
                            _ => {
                                println!("Unsupported channel count: {}", samples.channels);
                            }
                        }

                        self.streaming_source.queue_buffer(buffer).unwrap();
                    }
                    Ok(None) => {}
                    Err(e) => {
                        println!("Error: {:?}", e);
                        self.streaming_source.queue_buffer(buffer).unwrap();
                    }
                }
            }

            processed -= 1;
        }

        // The state changes when the buffers are exhausted
        if self.streaming_source.state() == alto::SourceState::Stopped {
            self.streaming_source.play();
        }
    }

    fn play(&mut self, looping: bool) {
        self.stop();

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

impl<T: Send + Sync> OpenAlAudioSource<T> {
    pub fn new(context: Arc<Context>) -> Self {
        let streaming_source = context.new_streaming_source().unwrap();

        Self {
            context,
            streaming_source,
            decoder: None,
            state: AudioSourceState::Stopped,
            looping: false,
            _marker: std::marker::PhantomData,
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

                    match buffer.unwrap() {
                        Ok(buffer) => self.streaming_source.queue_buffer(buffer).unwrap(),
                        Err(e) => {
                            log::error!("Audio: error creating buffer: {:?}", e);
                        }
                    }
                }
                _ => break,
            }
        }

        self.streaming_source.play();
        self.state = AudioSourceState::Playing;
    }
}

struct _MemorySource;
type OpenAlAudioMemorySource = OpenAlAudioSource<_MemorySource>;

impl AudioMemorySource for OpenAlAudioMemorySource {
    fn set_data(&mut self, data: Vec<u8>, codec_hint: Codec) {
        self.decoder = Some(create_decoder(data, codec_hint));
    }
}

struct _CustomDecoderSource;
type OpenAlAudioCustomDecoderSource = OpenAlAudioSource<_CustomDecoderSource>;

impl AudioCustomDecoderSource for OpenAlAudioCustomDecoderSource {
    fn set_decoder(&mut self, reader: Box<dyn super::decoders::Decoder>) {
        self.decoder = Some(reader);
    }
}

fn create_buffer_from_samples(
    samples: Samples,
    context: &Context,
) -> Option<AltoResult<alto::Buffer>> {
    match samples.channels {
        1 => Some(context.new_buffer::<Mono<i16>, _>(samples.data, samples.sample_rate)),
        2 => Some(context.new_buffer::<Stereo<i16>, _>(samples.data, samples.sample_rate)),
        _ => None,
    }
}

fn create_decoder(data: Vec<u8>, codec: Codec) -> Box<dyn Decoder> {
    match codec {
        Codec::Mp3 => Box::new(SymphoniaDecoder::new(data)),
        Codec::Ogg => Box::new(OggDecoder::new(data)),
        Codec::Wav => Box::new(WavDecoder::new(data)),
    }
}
