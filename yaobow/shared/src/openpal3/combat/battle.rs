use crate::openpal3::states::stats::{Attributes, Resources};

use super::{
    CombatRng,
    ai::{
        AiError, EnemyAiProfile, choose_noncasting_physical, choose_noncasting_physical_with_rest,
    },
    native_integer, physical_base_delta,
    resources::CombatVitals,
    timeline::{ACTOR_SLOTS, InitiativeTimeline, Opening, READY_THRESHOLD},
};

#[derive(Debug, Clone)]
pub struct PhysicalActor {
    pub level: i32,
    pub attributes: Attributes,
    pub vitals: CombatVitals,
    pub normal_attack: u32,
    pub qi_gain: i32,
    pub hit_divisor: i32,
    pub reduced_pattern: bool,
    pub double_damage: bool,
    pub reflect: bool,
    pub physical_barrier: bool,
    pub cannot_evade: bool,
    pub counter: bool,
    pub critical_bonus_percent: f32,
    pub unsupported_equipment: bool,
    /// Native packed formation/element flags, not the GDB stat affinities.
    pub element_table: [u32; 5],
}

impl PhysicalActor {
    pub fn new(level: i32, attributes: Attributes, current: Resources) -> Self {
        Self {
            level,
            attributes,
            vitals: CombatVitals::new(
                Resources {
                    hp: attributes.hp,
                    qi: attributes.qi,
                    mp: attributes.mp,
                },
                current,
            ),
            normal_attack: 0,
            qi_gain: 0,
            hit_divisor: 1,
            reduced_pattern: false,
            double_damage: false,
            reflect: false,
            physical_barrier: false,
            cannot_evade: false,
            counter: false,
            critical_bonus_percent: 0.0,
            unsupported_equipment: false,
            element_table: [0; 5],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackOutcome {
    Hit,
    Miss,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackError {
    InvalidActor,
    InvalidTarget,
    UnsupportedOnHitEffect,
    UnsupportedEquipment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackReport {
    pub outcome: AttackOutcome,
    /// The action-wide critical roll can succeed even when the target evades.
    pub critical: bool,
    pub evaded: bool,
    pub reflected: bool,
    pub recipient: Option<usize>,
    pub hp_change: i32,
    pub qi_change: i32,
}

/// Headless ordinary physical combat state. Command scheduling, enemy decisions,
/// special/on-hit effects and SCE/world lifecycle are separate integration layers.
pub struct BattleState {
    actors: [Option<PhysicalActor>; ACTOR_SLOTS],
    timeline: InitiativeTimeline,
    rng: CombatRng,
    money: i32,
}

impl BattleState {
    pub fn new(
        actors: [Option<PhysicalActor>; ACTOR_SLOTS],
        opening: Opening,
        seed: u32,
        money: i32,
    ) -> Self {
        Self::with_rng(actors, opening, CombatRng::new(seed), money)
    }

    pub(crate) fn with_rng(
        actors: [Option<PhysicalActor>; ACTOR_SLOTS],
        opening: Opening,
        mut rng: CombatRng,
        money: i32,
    ) -> Self {
        let speeds = std::array::from_fn(|slot| actors[slot].as_ref().map(|a| a.attributes.speed));
        let timeline = InitiativeTimeline::new(speeds, opening, &mut rng);
        Self {
            actors,
            timeline,
            rng,
            money,
        }
    }

    pub fn actor(&self, slot: usize) -> Option<&PhysicalActor> {
        self.actors.get(slot)?.as_ref()
    }

    pub fn timeline(&self) -> &InitiativeTimeline {
        &self.timeline
    }
    pub fn timeline_mut(&mut self) -> &mut InitiativeTimeline {
        &mut self.timeline
    }
    pub fn rng(&self) -> &CombatRng {
        &self.rng
    }
    pub fn money(&self) -> i32 {
        self.money
    }

    pub fn choose_enemy_physical(
        &mut self,
        source: usize,
        profile: &EnemyAiProfile,
    ) -> Result<Option<usize>, AiError> {
        choose_noncasting_physical(&self.actors, source, profile, &mut self.rng)
    }

    pub fn choose_enemy_physical_with_rest(
        &mut self,
        source: usize,
        profile: &EnemyAiProfile,
        at_rest: [bool; ACTOR_SLOTS],
    ) -> Result<Option<usize>, AiError> {
        choose_noncasting_physical_with_rest(&self.actors, source, profile, &mut self.rng, at_rest)
    }

    /// Resolves effects after preparation, not player-command submission.
    /// Actor attributes are effective stats; weapon on-hit effects are not supported.
    pub fn resolve_physical(
        &mut self,
        source: usize,
        target: usize,
    ) -> Result<AttackReport, AttackError> {
        self.validate_physical(source, target)?;
        let attacker = self.actor(source).ok_or(AttackError::InvalidActor)?.clone();
        let defender = self
            .actor(target)
            .ok_or(AttackError::InvalidTarget)?
            .clone();
        let critical_chance = if attacker.attributes.luck > 0 {
            (f64::from(attacker.attributes.luck / 2).sqrt() * 10.0) as i32
        } else {
            0
        };
        let critical = self.roll_percent(critical_chance);
        let mut report = AttackReport {
            outcome: AttackOutcome::Blocked,
            critical,
            evaded: false,
            reflected: false,
            recipient: None,
            hp_change: 0,
            qi_change: 0,
        };
        if !defender.vitals.enabled || defender.physical_barrier {
            return Ok(report);
        }
        // Native enemies do not make the party's speed-based evasion roll.
        let mut evaded = target < 5
            && self.roll_percent(
                defender
                    .attributes
                    .speed
                    .wrapping_sub(attacker.attributes.speed)
                    .wrapping_mul(5)
                    .max(0),
            );
        if defender.cannot_evade || defender.reflect {
            evaded = false;
        }
        report.evaded = evaded;
        let mut delta = if defender.vitals.invulnerable {
            0
        } else {
            let mut delta = physical_base_delta(
                attacker.level,
                attacker.attributes.attack,
                defender.attributes.defense,
                Some(&mut self.rng),
            );
            if attacker.reduced_pattern {
                let offset = (self.rng.next_u15() % 5) as i32;
                let percent = if self.rng.next_u15() % 1024 >= 512 {
                    100 + offset
                } else {
                    100 - offset
                };
                delta = delta.wrapping_mul(percent - 45) / 100;
            }
            let adjustment = elemental_adjustment(&attacker.element_table, &defender.element_table);
            delta = native_integer(
                (f64::from(delta) * (1.0 - f64::from(0.05f32) * f64::from(adjustment))) as f32,
            );
            if source < 5 && attacker.hit_divisor > 1 {
                delta /= attacker.hit_divisor;
            }
            if attacker.double_damage {
                delta = delta.wrapping_mul(2);
            }
            if delta >= 0 {
                delta = (self.rng.next_u15() % 5 + 1) as i32;
            }
            delta
        };
        if evaded || delta >= 0 {
            report.outcome = AttackOutcome::Miss;
            return Ok(report);
        }
        report.outcome = AttackOutcome::Hit;
        if critical {
            delta = native_integer((f64::from(delta) * f64::from(1.3f32)) as f32);
            let bonus = native_integer(
                (f64::from(delta) * f64::from(attacker.critical_bonus_percent) * f64::from(0.01f32))
                    as f32,
            );
            delta = delta.wrapping_add(bonus);
        }
        // This draw occurs on every ordinary hit, even without a counter ability.
        let reflected = defender.reflect || (self.rng.next_u15() % 1024 < 41 && defender.counter);
        report.reflected = reflected;
        let recipient = if reflected { source } else { target };
        report.recipient = Some(recipient);
        if !reflected
            && self
                .timeline
                .meter(target)
                .is_some_and(|meter| meter.preparing && meter.action < READY_THRESHOLD)
        {
            self.timeline.cancel_action(target);
        }
        report.hp_change = self.apply_hp(recipient, delta);
        if !reflected {
            if target < 5 {
                let actor = self.actors[target].as_mut().unwrap();
                let before = actor.vitals.current.qi;
                actor.vitals.change_qi(attacker.qi_gain);
                report.qi_change = actor.vitals.current.qi.wrapping_sub(before);
            }
            if source >= 5 {
                // Native case 4 consumes RNG even when IDs 922..925 have no
                // additional effect. It uses target luck, not the case-3
                // party-level/minimum-luck formula.
                let chance = attacker
                    .attributes
                    .luck
                    .wrapping_sub(defender.attributes.luck)
                    .wrapping_add(5);
                self.roll_percent(chance);
            }
        }
        Ok(report)
    }

    pub(super) fn validate_physical(
        &self,
        source: usize,
        target: usize,
    ) -> Result<(), AttackError> {
        let attacker = self.actor(source).ok_or(AttackError::InvalidActor)?;
        let defender = self.actor(target).ok_or(AttackError::InvalidTarget)?;
        if !attacker.vitals.enabled || attacker.vitals.removed || attacker.vitals.current.hp <= 0 {
            return Err(AttackError::InvalidActor);
        }
        if (source < 5) == (target < 5)
            || defender.vitals.current.hp <= 0
            || defender.vitals.removed
        {
            return Err(AttackError::InvalidTarget);
        }
        if attacker.unsupported_equipment || defender.unsupported_equipment {
            return Err(AttackError::UnsupportedEquipment);
        }
        if source >= 5
            && (901..=929).contains(&attacker.normal_attack)
            && !(922..=925).contains(&attacker.normal_attack)
        {
            return Err(AttackError::UnsupportedOnHitEffect);
        }
        Ok(())
    }

    fn roll_percent(&mut self, chance: i32) -> bool {
        chance > 0
            && ((self.rng.next_u15() % 1024) as i32)
                < native_integer((f64::from(chance) * f64::from(10.24f32)) as f32)
    }

    fn apply_hp(&mut self, slot: usize, delta: i32) -> i32 {
        let actor = self.actors[slot].as_mut().unwrap();
        let before = actor.vitals.current.hp;
        actor.vitals.change_hp(delta, &mut self.money);
        let after = actor.vitals.current.hp;
        if before > 0 && after == 0 {
            actor.double_damage = false;
            actor.reflect = false;
            actor.physical_barrier = false;
            actor.cannot_evade = false;
            self.timeline.reset_progress(slot);
        }
        after.wrapping_sub(before)
    }
}

fn elemental_adjustment(source: &[u32; 5], target: &[u32; 5]) -> i32 {
    if !target.iter().any(|&value| value != 0) {
        return 0;
    }
    let Some(element) = source.iter().position(|&value| value & 0xffff0000 != 0) else {
        return 0;
    };
    let counter = [5, 1, 4, 2, 3][element] - 1;
    let weaker = [2, 4, 5, 3, 1][element] - 1;
    i32::from(target[counter] != 0) - i32::from(target[weaker] != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor() -> PhysicalActor {
        let mut actor = PhysicalActor::new(
            20,
            Attributes {
                hp: 10000,
                qi: 100,
                mp: 100,
                attack: 350,
                defense: 30,
                speed: 3,
                luck: 2,
                ..Default::default()
            },
            Resources {
                hp: 10000,
                qi: 0,
                mp: 100,
            },
        );
        actor.normal_attack = 924;
        actor.qi_gain = 8;
        actor
    }

    #[test]
    fn invalid_and_unsupported_actions_leave_everything_unchanged() {
        let mut actors = std::array::from_fn(|_| None);
        actors[0] = Some(actor());
        let mut enemy = actor();
        enemy.normal_attack = 901;
        actors[5] = Some(enemy);
        let mut battle = BattleState::new(actors, Opening::Normal, 1, 200);
        let rng = *battle.rng();
        assert_eq!(
            battle.resolve_physical(11, 0),
            Err(AttackError::InvalidActor)
        );
        assert_eq!(
            battle.resolve_physical(0, 1),
            Err(AttackError::InvalidTarget)
        );
        assert_eq!(
            battle.resolve_physical(5, 0),
            Err(AttackError::UnsupportedOnHitEffect)
        );
        assert_eq!(*battle.rng(), rng);
        assert_eq!(battle.actor(0).unwrap().vitals.current.hp, 10000);
        assert_eq!(battle.money(), 200);
    }

    #[test]
    fn hits_cancel_preparation_and_lethal_damage_resets_the_meter() {
        let mut actors = std::array::from_fn(|_| None);
        actors[0] = Some(actor());
        let mut enemy = actor();
        enemy.vitals.current.hp = 1;
        actors[5] = Some(enemy);
        let mut battle = BattleState::new(actors, Opening::Normal, 1, 200);
        battle.timeline_mut().begin_action(5, 35);
        let report = battle.resolve_physical(0, 5).unwrap();
        assert_eq!(report.outcome, AttackOutcome::Hit);
        assert_eq!(report.hp_change, -1);
        assert_eq!(battle.actor(5).unwrap().vitals.current.hp, 0);
        let meter = battle.timeline().meter(5).unwrap();
        assert_eq!((meter.ready, meter.action, meter.preparing), (0, 0, false));
    }

    #[test]
    fn reflection_redirects_damage_without_interrupting_or_charging_the_defender() {
        let mut actors = std::array::from_fn(|_| None);
        actors[0] = Some(actor());
        let mut enemy = actor();
        enemy.reflect = true;
        actors[5] = Some(enemy);
        let mut battle = BattleState::new(actors, Opening::Normal, 1, 200);
        battle.timeline_mut().begin_action(5, 35);
        let report = battle.resolve_physical(0, 5).unwrap();
        assert_eq!(report.outcome, AttackOutcome::Hit);
        assert!(report.reflected && report.hp_change < 0);
        assert_eq!(report.recipient, Some(0));
        assert_eq!(battle.actor(5).unwrap().vitals.current.hp, 10000);
        assert_eq!(battle.actor(5).unwrap().vitals.current.qi, 0);
        assert!(battle.timeline().meter(5).unwrap().preparing);
    }

    #[test]
    fn ordinary_on_hit_rng_uses_the_victim_not_other_party_members() {
        let mut actors = std::array::from_fn(|_| None);
        actors[0] = Some(actor());
        actors[5] = Some(actor());
        let mut baseline = BattleState::new(actors.clone(), Opening::Normal, 0, 200);
        let mut other = actor();
        other.level = 40;
        other.attributes.luck = 0;
        actors[1] = Some(other);
        let mut expanded = BattleState::new(actors, Opening::Normal, 0, 200);
        baseline.rng = CombatRng::new(0);
        expanded.rng = CombatRng::new(0);
        assert_eq!(
            baseline.resolve_physical(5, 0).unwrap(),
            expanded.resolve_physical(5, 0).unwrap()
        );
        assert_eq!(baseline.rng(), expanded.rng());
    }

    #[test]
    fn unsupported_equipment_on_either_participant_rejects_before_damage_or_rng() {
        for guarded in [0, 5] {
            let mut actors = std::array::from_fn(|_| None);
            actors[0] = Some(actor());
            actors[5] = Some(actor());
            actors[guarded].as_mut().unwrap().unsupported_equipment = true;
            let mut battle = BattleState::new(actors, Opening::PartyReady, 6, 200);
            let before = *battle.rng();
            assert_eq!(
                battle.resolve_physical(0, 5),
                Err(AttackError::UnsupportedEquipment)
            );
            assert_eq!(
                battle.resolve_physical(5, 0),
                Err(AttackError::UnsupportedEquipment)
            );
            assert_eq!(*battle.rng(), before);
            assert_eq!(battle.actor(0).unwrap().vitals.current.hp, 10000);
            assert_eq!(battle.actor(5).unwrap().vitals.current.hp, 10000);
        }
    }

    #[test]
    #[ignore = "requires PAL3_ATTACK_ORACLE"]
    fn full_original_physical_actions_match_resources_meter_effects_and_rng() {
        use serde::Deserialize;
        #[derive(Deserialize)]
        struct ActorInput {
            level: i32,
            hp: i32,
            qi: i32,
            attack: i32,
            defense: i32,
            speed: i32,
            luck: i32,
            normal_attack: u32,
            qi_gain: i32,
            divisor: i32,
            reduced: bool,
            defending: bool,
            cannot_evade: bool,
            double_damage: bool,
            reflect: bool,
            barrier: bool,
            invulnerable: bool,
            removed: bool,
            money_shield: bool,
            counter: bool,
            critical_bonus: f32,
            elements: [u32; 5],
            enabled: bool,
            preparing: bool,
        }
        #[derive(Deserialize)]
        struct Input {
            source: usize,
            target: usize,
            actors: [Option<ActorInput>; ACTOR_SLOTS],
            seed: u32,
            money: i32,
        }
        #[derive(Deserialize)]
        struct ActorResult {
            hp: i32,
            qi: i32,
            defending: bool,
            preparing: bool,
            ready: i32,
            action: i32,
        }
        #[derive(Deserialize)]
        struct ResultState {
            actors: [Option<ActorResult>; ACTOR_SLOTS],
            money: i32,
            seed: u32,
            draws: u32,
            critical: bool,
            evaded: bool,
            reflected: bool,
            missed: bool,
            blocked: bool,
        }
        #[derive(Deserialize)]
        struct Case {
            input: Input,
            result: ResultState,
        }
        #[derive(Deserialize)]
        struct Oracle {
            cases: Vec<Case>,
        }
        let oracle: Oracle = serde_json::from_slice(
            &std::fs::read(std::env::var("PAL3_ATTACK_ORACLE").unwrap()).unwrap(),
        )
        .unwrap();
        assert!(!oracle.cases.is_empty());
        for (index, case) in oracle.cases.iter().enumerate() {
            let actors = std::array::from_fn(|slot| {
                case.input.actors[slot].as_ref().map(|a| {
                    let mut result = actor();
                    result.level = a.level;
                    result.attributes.attack = a.attack;
                    result.attributes.defense = a.defense;
                    result.attributes.speed = a.speed;
                    result.attributes.luck = a.luck;
                    result.vitals.current.hp = a.hp;
                    result.vitals.current.qi = a.qi;
                    result.normal_attack = a.normal_attack;
                    result.qi_gain = a.qi_gain;
                    result.hit_divisor = a.divisor;
                    result.reduced_pattern = a.reduced;
                    result.vitals.defending = a.defending;
                    result.cannot_evade = a.cannot_evade;
                    result.double_damage = a.double_damage;
                    result.reflect = a.reflect;
                    result.physical_barrier = a.barrier;
                    result.vitals.invulnerable = a.invulnerable;
                    result.vitals.removed = a.removed;
                    result.vitals.money_shield = a.money_shield;
                    result.counter = a.counter;
                    result.critical_bonus_percent = a.critical_bonus;
                    result.element_table = a.elements;
                    result.vitals.enabled = a.enabled;
                    result
                })
            });
            let mut battle =
                BattleState::new(actors, Opening::Normal, case.input.seed, case.input.money);
            // The native action fixtures start after meter/weapon initialization.
            battle.rng = CombatRng::new(case.input.seed);
            for (slot, actor) in case.input.actors.iter().enumerate() {
                if let Some(actor) = actor {
                    battle.timeline.set_progress(
                        slot,
                        1000,
                        if actor.preparing { 500 } else { 0 },
                        actor.preparing,
                    );
                }
            }
            let report = battle
                .resolve_physical(case.input.source, case.input.target)
                .unwrap();
            assert_eq!(
                (
                    report.critical,
                    report.evaded,
                    report.reflected,
                    report.outcome == AttackOutcome::Miss,
                    report.outcome == AttackOutcome::Blocked
                ),
                (
                    case.result.critical,
                    case.result.evaded,
                    case.result.reflected,
                    case.result.missed,
                    case.result.blocked
                ),
                "rolls case {index}"
            );
            assert_eq!(
                (battle.money(), battle.rng().state(), battle.rng().draws()),
                (case.result.money, case.result.seed, case.result.draws),
                "RNG/money case {index}"
            );
            for (slot, expected) in case.result.actors.iter().enumerate() {
                if let Some(expected) = expected {
                    let actual = battle.actor(slot).unwrap();
                    let meter = battle.timeline().meter(slot).unwrap();
                    assert_eq!(
                        (
                            actual.vitals.current.hp,
                            actual.vitals.current.qi,
                            actual.vitals.defending,
                            meter.preparing,
                            meter.ready,
                            meter.action
                        ),
                        (
                            expected.hp,
                            expected.qi,
                            expected.defending,
                            expected.preparing,
                            expected.ready,
                            expected.action
                        ),
                        "actor case {index}, slot {slot}"
                    );
                }
            }
        }
        eprintln!(
            "Compared {} full native physical actions",
            oracle.cases.len()
        );
    }
}
