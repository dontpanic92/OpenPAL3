use super::{
    AudioCustomDecoderSource, AudioMemorySource, Codec,
    decoders::{Decoder, OggDecoder, Samples, SymphoniaDecoder, WavDecoder},
};
use super::{AudioEngine, AudioSource, AudioSourceState};
use alto::{Alto, AltoResult, Context, Mono, Source, Stereo};
use std::sync::{Arc, Mutex, Weak};

pub struct OpenAlAudioEngine {
    context: Arc<Context>,
    /// Every source minted by this engine. The engine `update`
    /// (called once per frame from `CoreRadianceEngine::update`)
    /// walks this list, drops dead entries, and ticks each live one
    /// so streaming OpenAL queues stay fed without per-caller
    /// bookkeeping. Mirrors the FMOD / Wwise pattern of one
    /// engine-level pump.
    ///
    /// Stored behind `Arc<Mutex<...>>` (not `Rc<RefCell<...>>`)
    /// because the public `AudioSource` trait requires `Send + Sync`
    /// — the video player ships its audio source to a background
    /// thread.
    sources: Mutex<Vec<Weak<dyn OpenAlSourceTickable>>>,
}

impl AudioEngine for OpenAlAudioEngine {
    fn create_source(&self) -> Box<dyn AudioMemorySource> {
        let inner = Arc::new(Mutex::new(OpenAlAudioMemorySource::new(
            self.context.clone(),
        )));
        self.sources
            .lock()
            .unwrap()
            .push(Arc::downgrade(&inner) as Weak<dyn OpenAlSourceTickable>);
        Box::new(EngineOwnedMemorySource { inner })
    }

    fn create_custom_decoder_source(&self) -> Box<dyn AudioCustomDecoderSource> {
        let inner = Arc::new(Mutex::new(OpenAlAudioCustomDecoderSource::new(
            self.context.clone(),
        )));
        self.sources
            .lock()
            .unwrap()
            .push(Arc::downgrade(&inner) as Weak<dyn OpenAlSourceTickable>);
        Box::new(EngineOwnedCustomDecoderSource { inner })
    }

    fn update(&self, _delta_sec: f32) {
        let mut sources = self.sources.lock().unwrap();
        sources.retain(|weak| {
            if let Some(strong) = weak.upgrade() {
                strong.tick();
                true
            } else {
                false
            }
        });
    }

    fn set_listener(&self, position: [f32; 3], forward: [f32; 3], up: [f32; 3]) {
        let _ = self.context.set_position(position);
        let _ = self.context.set_orientation((forward, up));
    }
}

impl OpenAlAudioEngine {
    pub fn new() -> Self {
        let alto = Alto::load_default().unwrap();
        let device = alto.open(None).unwrap();
        let context = Arc::new(device.new_context(None).unwrap());

        // Use the clamped linear distance model so a source's
        // `max_distance` is an audible cutoff: gain falls linearly from
        // full at `reference_distance` to silence at `max_distance`.
        // Non-spatial (head-locked, relative) sources sit at the
        // reference distance and so keep full gain — the BGM/UI case.
        context.set_distance_model(alto::DistanceModel::LinearClamped);

        Self {
            context,
            sources: Mutex::new(Vec::new()),
        }
    }
}

/// Internal trait the engine uses to tick a source through its
/// `Mutex` without naming the underlying `OpenAlAudioSource<T>`
/// phantom-marker generic.
///
/// **Re-entrancy contract:** a source's `update()` MUST NOT call
/// back into `AudioEngine::create_source` /
/// `create_custom_decoder_source` — that would re-lock
/// `OpenAlAudioEngine::sources` mid-tick. OpenAL update paths do
/// not need to spawn sources, so this invariant is trivially upheld
/// today. The engine also uses `try_lock` on each source so a
/// transient borrow elsewhere (the video player's audio thread
/// poking the source on its own cadence) doesn't block the main
/// thread — the next frame's tick picks it up.
trait OpenAlSourceTickable: Send + Sync {
    fn tick(&self);
}

impl<T: Send + Sync + 'static> OpenAlSourceTickable for Mutex<OpenAlAudioSource<T>> {
    fn tick(&self) {
        if let Ok(mut s) = self.try_lock() {
            s.update();
        }
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

    fn set_position(&mut self, position: [f32; 3]) {
        let _ = self.streaming_source.set_position(position);
    }

    fn set_gain(&mut self, gain: f32) {
        let _ = self.streaming_source.set_gain(gain);
    }

    fn set_relative(&mut self, relative: bool) {
        self.streaming_source.set_relative(relative);
    }

    fn set_reference_distance(&mut self, distance: f32) {
        let _ = self.streaming_source.set_reference_distance(distance);
    }

    fn set_rolloff_factor(&mut self, factor: f32) {
        let _ = self.streaming_source.set_rolloff_factor(factor);
    }

    fn set_max_distance(&mut self, distance: f32) {
        let _ = self.streaming_source.set_max_distance(distance);
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

/// Caller-visible handle returned from `AudioEngine::create_source`.
/// Forwards every `AudioSource` / `AudioMemorySource` call to the
/// `Arc<Mutex<OpenAlAudioSource<_MemorySource>>>` that
/// `OpenAlAudioEngine` keeps a `Weak` of — so when this handle
/// drops, the engine's `Weak::upgrade()` starts returning `None` and
/// the engine tick stops touching the source automatically.
struct EngineOwnedMemorySource {
    inner: Arc<Mutex<OpenAlAudioMemorySource>>,
}

impl AudioSource for EngineOwnedMemorySource {
    fn update(&mut self) {
        self.inner.lock().unwrap().update();
    }
    fn play(&mut self, looping: bool) {
        self.inner.lock().unwrap().play(looping);
    }
    fn restart(&mut self) {
        self.inner.lock().unwrap().restart();
    }
    fn pause(&mut self) {
        self.inner.lock().unwrap().pause();
    }
    fn resume(&mut self) {
        self.inner.lock().unwrap().resume();
    }
    fn stop(&mut self) {
        self.inner.lock().unwrap().stop();
    }
    fn state(&self) -> AudioSourceState {
        self.inner.lock().unwrap().state()
    }
    fn set_position(&mut self, position: [f32; 3]) {
        self.inner.lock().unwrap().set_position(position);
    }
    fn set_gain(&mut self, gain: f32) {
        self.inner.lock().unwrap().set_gain(gain);
    }
    fn set_relative(&mut self, relative: bool) {
        self.inner.lock().unwrap().set_relative(relative);
    }
    fn set_reference_distance(&mut self, distance: f32) {
        self.inner.lock().unwrap().set_reference_distance(distance);
    }
    fn set_rolloff_factor(&mut self, factor: f32) {
        self.inner.lock().unwrap().set_rolloff_factor(factor);
    }
    fn set_max_distance(&mut self, distance: f32) {
        self.inner.lock().unwrap().set_max_distance(distance);
    }
}

impl AudioMemorySource for EngineOwnedMemorySource {
    fn set_data(&mut self, data: Vec<u8>, codec_hint: Codec) {
        self.inner.lock().unwrap().set_data(data, codec_hint);
    }
}

/// Sibling of [`EngineOwnedMemorySource`] for custom-decoder sources.
struct EngineOwnedCustomDecoderSource {
    inner: Arc<Mutex<OpenAlAudioCustomDecoderSource>>,
}

impl AudioSource for EngineOwnedCustomDecoderSource {
    fn update(&mut self) {
        self.inner.lock().unwrap().update();
    }
    fn play(&mut self, looping: bool) {
        self.inner.lock().unwrap().play(looping);
    }
    fn restart(&mut self) {
        self.inner.lock().unwrap().restart();
    }
    fn pause(&mut self) {
        self.inner.lock().unwrap().pause();
    }
    fn resume(&mut self) {
        self.inner.lock().unwrap().resume();
    }
    fn stop(&mut self) {
        self.inner.lock().unwrap().stop();
    }
    fn state(&self) -> AudioSourceState {
        self.inner.lock().unwrap().state()
    }
    fn set_position(&mut self, position: [f32; 3]) {
        self.inner.lock().unwrap().set_position(position);
    }
    fn set_gain(&mut self, gain: f32) {
        self.inner.lock().unwrap().set_gain(gain);
    }
    fn set_relative(&mut self, relative: bool) {
        self.inner.lock().unwrap().set_relative(relative);
    }
    fn set_reference_distance(&mut self, distance: f32) {
        self.inner.lock().unwrap().set_reference_distance(distance);
    }
    fn set_rolloff_factor(&mut self, factor: f32) {
        self.inner.lock().unwrap().set_rolloff_factor(factor);
    }
    fn set_max_distance(&mut self, distance: f32) {
        self.inner.lock().unwrap().set_max_distance(distance);
    }
}

impl AudioCustomDecoderSource for EngineOwnedCustomDecoderSource {
    fn set_decoder(&mut self, reader: Box<dyn super::decoders::Decoder>) {
        self.inner.lock().unwrap().set_decoder(reader);
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
