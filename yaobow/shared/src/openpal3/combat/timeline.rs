use super::CombatRng;

pub const ACTOR_SLOTS: usize = 11;
pub const READY_THRESHOLD: i32 = 0x2cec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opening {
    Normal,
    PartyReady,
    EnemiesReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitiativeMeter {
    pub ready: i32,
    pub action: i32,
    pub ready_crossings: i32,
    pub ready_rate: u8,
    pub action_rate: u8,
    pub preparing: bool,
    held: bool,
}

pub struct InitiativeTimeline {
    meters: [Option<InitiativeMeter>; ACTOR_SLOTS],
    time: f32,
    running: bool,
    paused: bool,
}

impl InitiativeTimeline {
    /// Native 0x4dbb80: five party slots followed by six enemy slots.
    /// Speeds are already adjusted for equipment and active stat modifiers.
    pub fn new(speeds: [Option<i32>; ACTOR_SLOTS], opening: Opening, rng: &mut CombatRng) -> Self {
        let meters = std::array::from_fn(|slot| {
            let speed = speeds[slot]?;
            let rate = speed.wrapping_add(4).wrapping_mul(5) as u8;
            let ready = match opening {
                Opening::Normal => speed
                    .wrapping_mul(638)
                    .wrapping_add((rng.next_u15() % 638) as i32),
                Opening::PartyReady => {
                    if slot < 5 {
                        READY_THRESHOLD
                    } else {
                        0
                    }
                }
                Opening::EnemiesReady => {
                    if slot >= 5 {
                        READY_THRESHOLD
                    } else {
                        0
                    }
                }
            };
            Some(InitiativeMeter {
                ready,
                action: 0,
                ready_crossings: 0,
                ready_rate: rate,
                action_rate: rate,
                preparing: false,
                held: false,
            })
        });
        Self {
            meters,
            time: 0.0,
            running: false,
            paused: false,
        }
    }

    pub fn meter(&self, slot: usize) -> Option<&InitiativeMeter> {
        self.meters.get(slot)?.as_ref()
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn set_running(&mut self, running: bool) {
        if self.running != running {
            self.running = running;
            self.paused = !running;
        }
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    fn meter_mut(&mut self, slot: usize) -> Option<&mut InitiativeMeter> {
        self.meters.get_mut(slot)?.as_mut()
    }

    pub fn hold_actor(&mut self, slot: usize, held: bool) -> bool {
        let Some(meter) = self.meter_mut(slot) else {
            return false;
        };
        meter.held = held;
        true
    }

    pub fn set_speed(&mut self, slot: usize, effective_speed: i32) -> bool {
        let Some(meter) = self.meter_mut(slot) else {
            return false;
        };
        let rate = effective_speed.wrapping_add(4).wrapping_mul(5) as u8;
        meter.ready_rate = rate;
        meter.action_rate = rate;
        true
    }

    /// Native 0x4db730/0x4db780 preserve existing action progress.
    pub fn begin_action(&mut self, slot: usize, rate: u8) -> bool {
        let Some(meter) = self.meter_mut(slot) else {
            return false;
        };
        meter.ready = 0;
        meter.preparing = true;
        meter.action_rate = rate;
        true
    }

    pub fn set_progress(&mut self, slot: usize, ready: i32, action: i32, preparing: bool) -> bool {
        let Some(meter) = self.meter_mut(slot) else {
            return false;
        };
        meter.ready = ready;
        meter.action = action;
        meter.preparing = preparing;
        true
    }

    /// Native 0x4db8b0 restores half readiness after cancelled preparation.
    pub fn cancel_action(&mut self, slot: usize) -> bool {
        let Some(meter) = self.meter_mut(slot) else {
            return false;
        };
        if meter.preparing {
            meter.action = 0;
            meter.ready = 5750;
            meter.preparing = false;
        }
        true
    }

    pub fn reset_progress(&mut self, slot: usize) -> bool {
        self.set_progress(slot, 0, 0, false)
    }

    /// Native 0x4db5c0/0x4dba60 truncate meter progress on every tick.
    /// Paused, held and disabled actors do not accumulate elapsed time.
    /// The battle dispatcher must use a consistent tick cadence, not merge ticks.
    pub fn advance_to(&mut self, now: f32, enabled: [bool; ACTOR_SLOTS]) -> bool {
        if !now.is_finite() || now < self.time {
            return false;
        }
        let elapsed = now - self.time;
        self.time = now;
        for (slot, meter) in self.meters.iter_mut().enumerate() {
            let Some(meter) = meter else { continue };
            if !enabled[slot] {
                continue;
            }
            let elapsed = if self.running && !self.paused && !meter.held {
                elapsed
            } else {
                0.0
            };
            if meter.preparing {
                meter.action =
                    advance_meter(meter.action, elapsed, meter.action_rate).min(READY_THRESHOLD);
            } else {
                let old = meter.ready;
                meter.ready = advance_meter(old, elapsed, meter.ready_rate);
                if meter.ready >= READY_THRESHOLD {
                    if old < READY_THRESHOLD {
                        meter.ready_crossings = meter.ready_crossings.wrapping_add(1);
                    }
                    meter.ready = READY_THRESHOLD;
                }
            }
        }
        true
    }

    /// Native 0x4db7a0 estimates the soonest actor for targeting/selection.
    /// The caller excludes dead, incapacitated or otherwise ineligible actors.
    pub fn soonest_actor(&self, eligible: [bool; ACTOR_SLOTS]) -> Option<usize> {
        let mut best = 100_000_000.0f32;
        let mut selected = None;
        for (slot, meter) in self.meters.iter().enumerate() {
            let Some(meter) = meter.filter(|_| eligible[slot]) else {
                continue;
            };
            let (progress, rate, delay) = if meter.preparing {
                (meter.action, meter.action_rate, 0.0)
            } else {
                (meter.ready, meter.ready_rate, 10000.0)
            };
            let candidate = f64::from(READY_THRESHOLD.wrapping_sub(progress))
                / (f64::from(rate) * 50.0)
                + delay;
            // FCOM tests C0, including unordered; the stored best is f32 but
            // each candidate is compared before that store. Ties can change slot.
            if candidate.is_nan() || best.is_nan() || candidate < f64::from(best) {
                best = candidate as f32;
                selected = Some(slot);
            }
        }
        selected
    }
}

fn advance_meter(progress: i32, elapsed: f32, rate: u8) -> i32 {
    let value = f64::from(progress) + f64::from(elapsed) * 50.0 * f64::from(rate);
    // Native FTOL truncates to i64 and returns its low word, unlike FISTP i32.
    if !(-9223372036854775808.0..9223372036854775808.0).contains(&value) {
        0
    } else {
        value as i64 as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_hold_and_disable_discard_time_without_resume_catchup() {
        let mut rng = CombatRng::new(1);
        let mut speeds = [None; ACTOR_SLOTS];
        speeds[0] = Some(3);
        let mut timeline = InitiativeTimeline::new(speeds, Opening::Normal, &mut rng);
        timeline.set_running(true);
        let initial = timeline.meter(0).unwrap().ready;
        assert!(timeline.advance_to(0.125, [true; ACTOR_SLOTS]));
        assert_eq!(timeline.meter(0).unwrap().ready, initial + 218);
        timeline.set_paused(true);
        timeline.advance_to(1.0, [true; ACTOR_SLOTS]);
        assert_eq!(timeline.meter(0).unwrap().ready, initial + 218);
        timeline.set_paused(false);
        timeline.hold_actor(0, true);
        timeline.advance_to(1.125, [true; ACTOR_SLOTS]);
        assert_eq!(timeline.meter(0).unwrap().ready, initial + 218);
        timeline.hold_actor(0, false);
        timeline.advance_to(1.25, [true; ACTOR_SLOTS]);
        assert_eq!(timeline.meter(0).unwrap().ready, initial + 436);
        timeline.advance_to(1.375, [false; ACTOR_SLOTS]);
        timeline.advance_to(1.5, [true; ACTOR_SLOTS]);
        assert_eq!(timeline.meter(0).unwrap().ready, initial + 654);
        let before = *timeline.meter(0).unwrap();
        assert!(!timeline.advance_to(1.0, [true; ACTOR_SLOTS]));
        assert!(!timeline.advance_to(f32::NAN, [true; ACTOR_SLOTS]));
        assert_eq!(*timeline.meter(0).unwrap(), before);
    }

    #[test]
    fn readiness_preparation_and_cancel_preserve_native_phase_rules() {
        let mut rng = CombatRng::new(1);
        let mut speeds = [None; ACTOR_SLOTS];
        speeds[0] = Some(3);
        let mut timeline = InitiativeTimeline::new(speeds, Opening::PartyReady, &mut rng);
        timeline.set_running(true);
        timeline.set_progress(0, READY_THRESHOLD - 1, 0, false);
        timeline.advance_to(0.125, [true; ACTOR_SLOTS]);
        timeline.advance_to(1.0, [true; ACTOR_SLOTS]);
        assert_eq!(timeline.meter(0).unwrap().ready_crossings, 1);
        timeline.begin_action(0, 7);
        timeline.advance_to(1.125, [true; ACTOR_SLOTS]);
        assert_eq!(timeline.meter(0).unwrap().action, 43);
        timeline.cancel_action(0);
        assert_eq!(
            (
                timeline.meter(0).unwrap().ready,
                timeline.meter(0).unwrap().action
            ),
            (5750, 0)
        );
        timeline.cancel_action(0);
        timeline.set_progress(0, 5750, 99, false);
        timeline.begin_action(0, 255);
        assert_eq!(timeline.meter(0).unwrap().action, 99);
        timeline.advance_to(2.125, [true; ACTOR_SLOTS]);
        assert_eq!(timeline.meter(0).unwrap().action, READY_THRESHOLD);
        timeline.reset_progress(0);
        assert_eq!(timeline.meter(0).unwrap().ready_crossings, 1);
        assert!(!timeline.begin_action(11, 1));
        assert!(!timeline.hold_actor(1, true));
    }

    #[test]
    fn selection_preserves_native_float_store_ties_and_zero_rate_behavior() {
        let mut rng = CombatRng::new(1);
        let mut speeds = [None; ACTOR_SLOTS];
        speeds[..2].fill(Some(3));
        let mut timeline = InitiativeTimeline::new(speeds, Opening::Normal, &mut rng);
        for slot in 0..2 {
            timeline.begin_action(slot, 3);
            timeline.set_progress(slot, 0, READY_THRESHOLD - 1, true);
        }
        assert_eq!(timeline.soonest_actor([true; ACTOR_SLOTS]), Some(1));
        for slot in 0..2 {
            timeline.begin_action(slot, 1);
        }
        assert_eq!(timeline.soonest_actor([true; ACTOR_SLOTS]), Some(0));
        for slot in 0..2 {
            timeline.begin_action(slot, 0);
            timeline.set_progress(slot, 0, READY_THRESHOLD, true);
        }
        assert_eq!(timeline.soonest_actor([true; ACTOR_SLOTS]), Some(1));
        assert_eq!(timeline.soonest_actor([false; ACTOR_SLOTS]), None);
    }

    #[test]
    #[ignore = "requires PAL3_TIMELINE_ORACLE"]
    fn original_timeline_traces_match_initialization_ticks_and_selection() {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct Meter {
            ready: i32,
            action: i32,
            crossings: i32,
            ready_rate: u8,
            action_rate: u8,
            preparing: bool,
            held: bool,
        }
        #[derive(Deserialize)]
        struct Snapshot {
            meters: [Option<Meter>; ACTOR_SLOTS],
            selected: i32,
            running: bool,
            paused: bool,
            seed: u32,
            draws: u32,
        }
        #[derive(Deserialize)]
        #[serde(tag = "op", rename_all = "snake_case")]
        enum Operation {
            Running {
                value: u8,
            },
            Paused {
                value: u8,
            },
            Held {
                slot: usize,
                value: u8,
            },
            Advance {
                now: f32,
                enabled: [bool; ACTOR_SLOTS],
            },
            Begin {
                slot: usize,
                value: u8,
            },
            Cancel {
                slot: usize,
            },
            Reset {
                slot: usize,
            },
            Speed {
                slot: usize,
                value: i32,
            },
            Progress {
                slot: usize,
                ready: i32,
                action: i32,
                preparing: bool,
            },
            Eligible {
                eligible: [bool; ACTOR_SLOTS],
            },
        }
        #[derive(Deserialize)]
        struct Step {
            operation: Operation,
            result: Snapshot,
        }
        #[derive(Deserialize)]
        struct Case {
            speeds: [Option<i32>; ACTOR_SLOTS],
            opening: u8,
            seed: u32,
            initial: Snapshot,
            steps: Vec<Step>,
        }
        #[derive(Deserialize)]
        struct Oracle {
            cases: Vec<Case>,
        }

        fn check(
            timeline: &InitiativeTimeline,
            rng: &CombatRng,
            eligible: [bool; ACTOR_SLOTS],
            expected: &Snapshot,
            case: usize,
            step: usize,
        ) {
            for (slot, expected) in expected.meters.iter().enumerate() {
                match (timeline.meter(slot), expected) {
                    (Some(actual), Some(expected)) => assert_eq!(
                        (
                            actual.ready,
                            actual.action,
                            actual.ready_crossings,
                            actual.ready_rate,
                            actual.action_rate,
                            actual.preparing,
                            actual.held
                        ),
                        (
                            expected.ready,
                            expected.action,
                            expected.crossings,
                            expected.ready_rate,
                            expected.action_rate,
                            expected.preparing,
                            expected.held
                        ),
                        "case {case}, step {step}, slot {slot}"
                    ),
                    (None, None) => {}
                    _ => panic!("case {case}, step {step}, slot {slot}: presence mismatch"),
                }
            }
            assert_eq!(
                timeline
                    .soonest_actor(eligible)
                    .map_or(-1, |slot| slot as i32),
                expected.selected,
                "selection case {case}, step {step}"
            );
            assert_eq!(
                (
                    timeline.running(),
                    timeline.paused(),
                    rng.state(),
                    rng.draws()
                ),
                (
                    expected.running,
                    expected.paused,
                    expected.seed,
                    expected.draws
                ),
                "clock/RNG case {case}, step {step}"
            );
        }
        let oracle: Oracle = serde_json::from_slice(
            &std::fs::read(std::env::var("PAL3_TIMELINE_ORACLE").unwrap()).unwrap(),
        )
        .unwrap();
        assert!(!oracle.cases.is_empty());
        let mut snapshots = 0;
        for (case_index, case) in oracle.cases.iter().enumerate() {
            let mut rng = CombatRng::new(case.seed);
            let opening = if case.opening & 2 != 0 {
                Opening::EnemiesReady
            } else if case.opening & 1 != 0 {
                Opening::PartyReady
            } else {
                Opening::Normal
            };
            let mut timeline = InitiativeTimeline::new(case.speeds, opening, &mut rng);
            let mut eligible = [true; ACTOR_SLOTS];
            check(&timeline, &rng, eligible, &case.initial, case_index, 0);
            snapshots += 1;
            for (step_index, step) in case.steps.iter().enumerate() {
                match &step.operation {
                    Operation::Running { value } => timeline.set_running(*value != 0),
                    Operation::Paused { value } => timeline.set_paused(*value != 0),
                    Operation::Held { slot, value } => {
                        timeline.hold_actor(*slot, *value != 0);
                    }
                    Operation::Advance { now, enabled } => {
                        assert!(timeline.advance_to(*now, *enabled));
                    }
                    Operation::Begin { slot, value } => {
                        timeline.begin_action(*slot, *value);
                    }
                    Operation::Cancel { slot } => {
                        timeline.cancel_action(*slot);
                    }
                    Operation::Reset { slot } => {
                        timeline.reset_progress(*slot);
                    }
                    Operation::Speed { slot, value } => {
                        timeline.set_speed(*slot, *value);
                    }
                    Operation::Progress {
                        slot,
                        ready,
                        action,
                        preparing,
                    } => {
                        timeline.set_progress(*slot, *ready, *action, *preparing);
                    }
                    Operation::Eligible { eligible: value } => eligible = *value,
                }
                check(
                    &timeline,
                    &rng,
                    eligible,
                    &step.result,
                    case_index,
                    step_index + 1,
                );
                snapshots += 1;
            }
        }
        eprintln!(
            "Compared {} native timeline traces and {snapshots} snapshots",
            oracle.cases.len()
        );
    }
}
