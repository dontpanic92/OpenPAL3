//! Generic engine-backed audio node component.
//!
//! [`AudioSourceComponent`] turns any entity into an audio emitter. It
//! owns an [`AudioMemorySource`] minted from the [`AudioEngine`] and,
//! when [`spatial`](AudioNodeConfig::spatial) is enabled, positions that
//! source at the entity's world transform every frame so OpenAL applies
//! distance attenuation and stereo panning relative to the listener (the
//! active scene camera, pushed once per frame by
//! `CoreRadianceEngine::update`). Non-spatial nodes play head-locked
//! (relative to the listener, full gain, no panning) — the right
//! behaviour for BGM / UI cues.
//!
//! The component also models three playback shapes, so callers don't
//! have to drive scheduling themselves:
//!
//! * [`PlaybackMode::Loop`] — seamless native-looping ambience
//!   (river / waterfall). Started once on load.
//! * [`PlaybackMode::OneShot`] — play once on load, then go silent.
//! * [`PlaybackMode::RandomInterval`] — intermittent one-shots
//!   re-triggered on a random `[min, max]` countdown (birds, creaks).
//!   The first fire is phased over `[0, max]` to stagger dense scenes
//!   that share a period, and the countdown is frozen while this node's
//!   own previous instance is still audible so it never stacks copies of
//!   itself.
//!
//! The component **self-ticks** via [`IComponent::on_updating`]; the
//! owning container dispatches it every frame while the entity is
//! active, so callers attach once and do nothing else. On
//! [`IComponent::on_unloading`] (entity removed / scene swapped) the
//! source is stopped, so seamless ambient beds tear down with their
//! entity.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crosscom::ComRc;

use crate::audio::{AudioEngine, AudioMemorySource, AudioSourceState, Codec};
use crate::comdef::{IAudioSourceComponentImpl, IComponentImpl, IEntity, IEntityExt};
use crate::math::Vec3;

ComObject_AudioSourceComponent!(super::AudioSourceComponent);

fn vec3_to_array(v: Vec3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

/// How an [`AudioSourceComponent`] schedules playback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlaybackMode {
    /// Play once with native looping the moment the node loads; never
    /// re-triggered. Used for seamless ambient beds.
    Loop,
    /// Play once on load, then stay silent.
    OneShot,
    /// Intermittent one-shots re-triggered on a random `[min, max]`
    /// second countdown. `min`/`max` are sanitised at construction so
    /// `0 < min <= max`.
    RandomInterval { min: f32, max: f32 },
}

/// Construction-time configuration for an [`AudioSourceComponent`].
#[derive(Clone, Copy, Debug)]
pub struct AudioNodeConfig {
    /// When `true`, the source is positioned in 3D at the owning
    /// entity's world transform and OpenAL attenuation/panning applies.
    /// When `false`, the source plays head-locked (relative to the
    /// listener, no attenuation) — the BGM / UI case.
    pub spatial: bool,
    /// Scheduling shape. See [`PlaybackMode`].
    pub mode: PlaybackMode,
    /// Linear gain applied to the source (`1.0` = unchanged).
    pub gain: f32,
    /// OpenAL reference distance: within this distance the source plays
    /// at full `gain`; beyond it attenuates per the rolloff model.
    pub reference_distance: f32,
    /// OpenAL rolloff factor (`1.0` = default inverse-distance falloff;
    /// larger = steeper).
    pub rolloff_factor: f32,
    /// OpenAL max distance: attenuation stops decreasing past this
    /// distance (an effective audible cutoff for the clamped models).
    pub max_distance: f32,
}

impl Default for AudioNodeConfig {
    fn default() -> Self {
        Self {
            spatial: true,
            mode: PlaybackMode::Loop,
            gain: 1.0,
            reference_distance: 1.0,
            rolloff_factor: 1.0,
            max_distance: f32::MAX,
        }
    }
}

/// Floor for random replay intervals, mirroring PAL4's emitter guard:
/// defends against malformed `0` / negative / NaN values that would
/// otherwise burst-fire every frame.
const MIN_REPLAY_INTERVAL_SEC: f32 = 0.1;

/// Sanitise a `[min, max]` replay interval to finite, non-negative
/// values with `min >= MIN_REPLAY_INTERVAL_SEC` and `max >= min`.
pub fn sanitise_interval(min: f32, max: f32) -> (f32, f32) {
    fn clean(v: f32) -> f32 {
        if v.is_finite() && v > 0.0 {
            v.max(MIN_REPLAY_INTERVAL_SEC)
        } else {
            MIN_REPLAY_INTERVAL_SEC
        }
    }
    let lo = clean(min);
    let hi = clean(max);
    if hi < lo { (lo, lo) } else { (lo, hi) }
}

/// A uniform random sample in `[min, max]`. A tiny xorshift keeps the
/// component free of an `rand` dependency and deterministic per-process
/// seed isn't required (jitter only needs to look unstructured).
fn uniform(min: f32, max: f32) -> f32 {
    if max <= min {
        return min;
    }
    use std::cell::Cell;
    thread_local! {
        static STATE: Cell<u64> = const { Cell::new(0x9e3779b97f4a7c15) };
    }
    let bits = STATE.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        x
    });
    let unit = (bits >> 40) as f32 / (1u32 << 24) as f32; // [0,1)
    min + unit * (max - min)
}

pub struct AudioSourceComponent {
    entity: ComRc<IEntity>,
    source: RefCell<Box<dyn AudioMemorySource>>,
    config: AudioNodeConfig,
    /// Seconds until the next random-interval fire. Unused for
    /// `Loop` / `OneShot`.
    next_play_in_sec: Cell<f32>,
}

impl AudioSourceComponent {
    /// Build an audio node from raw encoded audio `data` (decoded with
    /// `codec`). The source is minted from `audio_engine`; spatial
    /// parameters are applied at construction so they're live before the
    /// first frame. Playback starts in [`IComponent::on_loading`].
    pub fn create(
        entity: ComRc<IEntity>,
        audio_engine: Rc<dyn AudioEngine>,
        data: Vec<u8>,
        codec: Codec,
        config: AudioNodeConfig,
    ) -> ComRc<crate::comdef::IAudioSourceComponent> {
        let mut source = audio_engine.create_source();
        source.set_data(data, codec);
        source.set_gain(config.gain);
        if config.spatial {
            source.set_relative(false);
            source.set_reference_distance(config.reference_distance);
            source.set_rolloff_factor(config.rolloff_factor);
            source.set_max_distance(config.max_distance);
            source.set_position(vec3_to_array(entity.world_transform().position()));
        } else {
            // Head-locked: play at the listener with no attenuation.
            source.set_relative(true);
            source.set_position([0.0, 0.0, 0.0]);
        }

        let next_play_in_sec = match config.mode {
            // Phase the first intermittent fire over [0, max].
            PlaybackMode::RandomInterval { max, .. } => uniform(0.0, max),
            _ => 0.0,
        };

        ComRc::from_object(Self {
            entity,
            source: RefCell::new(source),
            config,
            next_play_in_sec: Cell::new(next_play_in_sec),
        })
    }

    fn sync_position(&self) {
        if !self.config.spatial {
            return;
        }
        let pos = self.entity.world_transform().position();
        self.source.borrow_mut().set_position(vec3_to_array(pos));
    }
}

impl IAudioSourceComponentImpl for AudioSourceComponent {
    fn play(&self) -> crosscom::Void {
        let looping = self.config.mode == PlaybackMode::Loop;
        self.source.borrow_mut().play(looping);
    }

    fn stop(&self) -> crosscom::Void {
        self.source.borrow_mut().stop();
    }

    fn set_gain(&self, gain: f32) -> crosscom::Void {
        self.source.borrow_mut().set_gain(gain);
    }
}

impl IComponentImpl for AudioSourceComponent {
    fn on_loading(&self) -> crosscom::Void {
        self.sync_position();
        match self.config.mode {
            PlaybackMode::Loop => self.source.borrow_mut().play(true),
            PlaybackMode::OneShot | PlaybackMode::RandomInterval { .. } => {
                // RandomInterval waits for its first countdown in
                // on_updating; OneShot fires immediately.
                if matches!(self.config.mode, PlaybackMode::OneShot) {
                    self.source.borrow_mut().play(false);
                }
            }
        }
    }

    fn on_updating(&self, delta_sec: f32) -> crosscom::Void {
        self.sync_position();

        let (min, max) = match self.config.mode {
            PlaybackMode::RandomInterval { min, max } => (min, max),
            // Loop / OneShot have no countdown; position sync above is
            // all the per-frame work they need.
            _ => return,
        };

        // Freeze the countdown while this node's own previous instance
        // is still audible, so it never stacks overlapping copies.
        if self.source.borrow().state() != AudioSourceState::Stopped {
            return;
        }

        let remaining = self.next_play_in_sec.get() - delta_sec;
        if remaining > 0.0 {
            self.next_play_in_sec.set(remaining);
            return;
        }
        self.next_play_in_sec.set(uniform(min, max));
        self.sync_position();
        self.source.borrow_mut().play(false);
    }

    fn on_unloading(&self) {
        self.source.borrow_mut().stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitise_interval_clamps_zero_and_orders() {
        assert_eq!(sanitise_interval(0.0, 0.0), (0.1, 0.1));
        assert_eq!(sanitise_interval(5.0, 10.0), (5.0, 10.0));
        // max < min collapses to a fixed period at min.
        assert_eq!(sanitise_interval(8.0, 2.0), (8.0, 8.0));
        // NaN / negative fall back to the floor.
        assert_eq!(sanitise_interval(f32::NAN, -1.0), (0.1, 0.1));
    }

    #[test]
    fn uniform_stays_in_range() {
        for _ in 0..1000 {
            let v = uniform(2.0, 5.0);
            assert!(v >= 2.0 && v <= 5.0, "out of range: {}", v);
        }
        assert_eq!(uniform(3.0, 3.0), 3.0);
    }
}
