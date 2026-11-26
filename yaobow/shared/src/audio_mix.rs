use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use radiance::audio::{AudioMemorySource, AudioSource, AudioSourceState, Codec};

#[derive(Copy, Clone)]
pub enum AudioChannel {
    Music,
    Sound,
}

pub struct AudioMix {
    music: AtomicU32,
    sound: AtomicU32,
}

impl AudioMix {
    pub fn new(music: f32, sound: f32) -> Self {
        Self {
            music: AtomicU32::new(sanitize_volume(music, 1.0).to_bits()),
            sound: AtomicU32::new(sanitize_volume(sound, 1.0).to_bits()),
        }
    }

    pub fn from_config(config: &crate::config::YaobowConfig) -> Self {
        Self::new(config.music_volume(), config.sound_volume())
    }

    pub fn load() -> Self {
        Self::from_config(&crate::config::YaobowConfig::load())
    }

    pub fn music(&self) -> f32 {
        f32::from_bits(self.music.load(Ordering::Relaxed))
    }

    pub fn sound(&self) -> f32 {
        f32::from_bits(self.sound.load(Ordering::Relaxed))
    }

    pub fn set_music(&self, volume: f32) -> bool {
        let Some(volume) = bounded_volume(volume) else {
            return false;
        };
        self.music.store(volume.to_bits(), Ordering::Relaxed);
        true
    }

    pub fn set_sound(&self, volume: f32) -> bool {
        let Some(volume) = bounded_volume(volume) else {
            return false;
        };
        self.sound.store(volume.to_bits(), Ordering::Relaxed);
        true
    }

    fn volume(&self, channel: AudioChannel) -> f32 {
        match channel {
            AudioChannel::Music => self.music(),
            AudioChannel::Sound => self.sound(),
        }
    }
}

impl Default for AudioMix {
    fn default() -> Self {
        Self::new(1.0, 1.0)
    }
}

pub fn mixed_source(
    source: Box<dyn AudioMemorySource>,
    mix: Arc<AudioMix>,
    channel: AudioChannel,
) -> Box<dyn AudioMemorySource> {
    Box::new(MixedAudioSource::new(source, mix, channel))
}

fn sanitize_volume(volume: f32, fallback: f32) -> f32 {
    bounded_volume(volume).unwrap_or(fallback)
}

fn bounded_volume(volume: f32) -> Option<f32> {
    volume.is_finite().then(|| volume.clamp(0.0, 1.0))
}

struct MixedAudioSource {
    source: Box<dyn AudioMemorySource>,
    mix: Arc<AudioMix>,
    channel: AudioChannel,
    source_gain: f32,
    applied_gain: f32,
}

impl MixedAudioSource {
    fn new(source: Box<dyn AudioMemorySource>, mix: Arc<AudioMix>, channel: AudioChannel) -> Self {
        let mut mixed = Self {
            source,
            mix,
            channel,
            source_gain: 1.0,
            applied_gain: f32::NAN,
        };
        mixed.apply_gain();
        mixed
    }

    fn apply_gain(&mut self) {
        let gain = self.source_gain * self.mix.volume(self.channel);
        if gain != self.applied_gain {
            self.source.set_gain(gain);
            self.applied_gain = gain;
        }
    }
}

impl AudioSource for MixedAudioSource {
    fn update(&mut self) {
        self.apply_gain();
        self.source.update();
    }

    fn play(&mut self, looping: bool) {
        self.apply_gain();
        self.source.play(looping);
    }

    fn restart(&mut self) {
        self.apply_gain();
        self.source.restart();
    }

    fn pause(&mut self) {
        self.source.pause();
    }

    fn resume(&mut self) {
        self.apply_gain();
        self.source.resume();
    }

    fn stop(&mut self) {
        self.source.stop();
    }

    fn state(&self) -> AudioSourceState {
        self.source.state()
    }

    fn set_position(&mut self, position: [f32; 3]) {
        self.apply_gain();
        self.source.set_position(position);
    }

    fn set_gain(&mut self, gain: f32) {
        if gain.is_finite() {
            self.source_gain = gain.max(0.0);
        }
        self.apply_gain();
    }

    fn set_relative(&mut self, relative: bool) {
        self.source.set_relative(relative);
    }

    fn set_reference_distance(&mut self, distance: f32) {
        self.source.set_reference_distance(distance);
    }

    fn set_rolloff_factor(&mut self, factor: f32) {
        self.source.set_rolloff_factor(factor);
    }

    fn set_max_distance(&mut self, distance: f32) {
        self.source.set_max_distance(distance);
    }
}

impl AudioMemorySource for MixedAudioSource {
    fn set_data(&mut self, data: Vec<u8>, codec_hint: Codec) {
        self.source.set_data(data, codec_hint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeState {
        gains: Vec<f32>,
        state: Option<AudioSourceState>,
    }

    struct FakeSource(Arc<Mutex<FakeState>>);

    impl AudioSource for FakeSource {
        fn update(&mut self) {}
        fn play(&mut self, _looping: bool) {
            self.0.lock().unwrap().state = Some(AudioSourceState::Playing);
        }
        fn restart(&mut self) {}
        fn pause(&mut self) {}
        fn resume(&mut self) {}
        fn stop(&mut self) {
            self.0.lock().unwrap().state = Some(AudioSourceState::Stopped);
        }
        fn state(&self) -> AudioSourceState {
            self.0
                .lock()
                .unwrap()
                .state
                .unwrap_or(AudioSourceState::Stopped)
        }
        fn set_gain(&mut self, gain: f32) {
            self.0.lock().unwrap().gains.push(gain);
        }
    }

    impl AudioMemorySource for FakeSource {
        fn set_data(&mut self, _data: Vec<u8>, _codec_hint: Codec) {}
    }

    fn fake() -> (Box<dyn AudioMemorySource>, Arc<Mutex<FakeState>>) {
        let state = Arc::new(Mutex::new(FakeState::default()));
        (Box::new(FakeSource(state.clone())), state)
    }

    #[test]
    fn channel_mix_is_independent_for_current_and_future_sources() {
        let mix = Arc::new(AudioMix::default());
        let (music, music_state) = fake();
        let (sound, sound_state) = fake();
        let mut music = mixed_source(music, mix.clone(), AudioChannel::Music);
        let mut sound = mixed_source(sound, mix.clone(), AudioChannel::Sound);

        assert!(mix.set_music(0.25));
        music.update();
        sound.update();
        assert_eq!(music_state.lock().unwrap().gains.last(), Some(&0.25));
        assert_eq!(sound_state.lock().unwrap().gains.last(), Some(&1.0));

        assert!(mix.set_sound(0.6));
        sound.update();
        let (future, future_state) = fake();
        let _future = mixed_source(future, mix, AudioChannel::Sound);
        assert_eq!(sound_state.lock().unwrap().gains.last(), Some(&0.6));
        assert_eq!(future_state.lock().unwrap().gains.last(), Some(&0.6));
    }

    #[test]
    fn channel_mix_multiplies_transient_source_gain() {
        let mix = Arc::new(AudioMix::new(0.5, 1.0));
        let (source, state) = fake();
        let mut source = mixed_source(source, mix.clone(), AudioChannel::Music);

        source.set_gain(0.4);
        assert_eq!(state.lock().unwrap().gains.last(), Some(&0.2));
        assert!(mix.set_music(0.25));
        source.update();
        assert_eq!(state.lock().unwrap().gains.last(), Some(&0.1));
    }

    #[test]
    fn non_finite_channel_values_are_rejected_without_changing_mix() {
        let mix = AudioMix::new(0.3, 0.7);
        assert!(!mix.set_music(f32::NAN));
        assert!(!mix.set_sound(f32::INFINITY));
        assert_eq!(mix.music(), 0.3);
        assert_eq!(mix.sound(), 0.7);
    }
}
