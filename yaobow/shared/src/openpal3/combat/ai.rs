use fileformats::pal3::gdb::RoleDefinition;

use super::{CombatRng, battle::PhysicalActor, timeline::ACTOR_SLOTS};

#[derive(Debug, Clone)]
pub struct EnemyAiProfile {
    pub role_id: u32,
    pub spells: [u32; 4],
    pub uses_items: bool,
}

impl EnemyAiProfile {
    pub fn from_role(role: &RoleDefinition) -> Self {
        Self {
            role_id: role.id,
            spells: role.spells,
            uses_items: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiError {
    InvalidActor,
    UnsupportedCasting,
    UnsupportedItems,
    UnsupportedBoss,
    InvalidVitals,
    UnsupportedStatusOrFormation,
    UnsupportedOnHitEffect,
    UnsupportedPresentation,
}

/// Native 0x4d7120 selects healthy/wounded/critical non-casting decision trees.
/// No decision is a real native outcome; it does not consume the ready meter.
/// All actor presentations must be at rest. Other decision trees/capabilities
/// are rejected without advancing RNG.
pub fn choose_noncasting_physical(
    actors: &[Option<PhysicalActor>; ACTOR_SLOTS],
    source: usize,
    profile: &EnemyAiProfile,
    rng: &mut CombatRng,
) -> Result<Option<usize>, AiError> {
    let actor = actors
        .get(source)
        .and_then(Option::as_ref)
        .ok_or(AiError::InvalidActor)?;
    if source < 5 || !actor.vitals.enabled || actor.vitals.current.hp <= 0 {
        return Err(AiError::InvalidActor);
    }
    if profile.role_id == 739 {
        return Err(AiError::UnsupportedBoss);
    }
    if profile.spells.iter().any(|&id| id != 0) {
        return Err(AiError::UnsupportedCasting);
    }
    if profile.uses_items {
        return Err(AiError::UnsupportedItems);
    }
    if (901..=929).contains(&actor.normal_attack) && !(922..=925).contains(&actor.normal_attack) {
        return Err(AiError::UnsupportedOnHitEffect);
    }
    for actor in actors.iter().flatten() {
        if actor.vitals.maximum.hp <= 0
            || actor.vitals.current.hp < 0
            || (actor.vitals.current.hp == 0 && actor.vitals.enabled)
            || actor.vitals.current.hp > actor.vitals.maximum.hp
        {
            return Err(AiError::InvalidVitals);
        }
        if actor.vitals.removed
            || actor.vitals.invulnerable
            || actor.physical_barrier
            || actor.reflect
            || actor.cannot_evade
            || actor.double_damage
            || actor.element_table.iter().any(|&value| value != 0)
        {
            return Err(AiError::UnsupportedStatusOrFormation);
        }
    }
    let source_hp = actor.vitals.current.hp;
    let source_max = actor.vitals.maximum.hp;
    let healthy = source_hp >= source_max / 2;
    let critical = !healthy && source_hp < source_max / 4;
    // Cache 0x4d94f0 counts present party members in bands 3..5, including
    // disabled dead actors. Two or more enable an area-action attempt.
    let party_needs_area_action = actors[..5]
        .iter()
        .flatten()
        .filter(|actor| actor.vitals.current.hp < actor.vitals.maximum.hp / 2)
        .count()
        >= 2;
    // The original samples each slot in order, including absent slots, rather
    // than choosing uniformly among living party members.
    let mut start = ACTOR_SLOTS;
    for (slot, actor) in actors.iter().enumerate() {
        if rng.next_u15() % 3 == 1 && actor.is_some() {
            start = slot;
            break;
        }
    }
    if start == 10 {
        start = 0;
    }
    for (slot, actor) in actors.iter().enumerate().skip(start) {
        let Some(actor) = actor else { continue };
        if !actor.vitals.enabled {
            continue;
        }
        // The critical tree tries an ordinary hit before any spell attempt.
        if critical && slot < 5 {
            return Ok(Some(slot));
        }
        if party_needs_area_action {
            // The area spell/item attempt cannot succeed in this profile, but
            // still advances RNG once in every enabled candidate's traversal.
            rng.next_u15();
        }
        if slot < 5 {
            // Healthy and wounded trees try a 15%/10% spell action before
            // falling back to physical. Unavailable casting still draws.
            rng.next_u15();
            return Ok(Some(slot));
        }
        // Only the healthy tree has these nested enemy-side 30% attempts.
        if healthy && rng.next_u15() % 1024 < 307 {
            rng.next_u15();
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openpal3::states::stats::{Attributes, Resources};

    fn actor() -> PhysicalActor {
        let mut actor = PhysicalActor::new(
            20,
            Attributes {
                hp: 100,
                qi: 100,
                mp: 100,
                attack: 30,
                defense: 10,
                speed: 3,
                luck: 2,
                ..Default::default()
            },
            Resources {
                hp: 100,
                qi: 0,
                mp: 0,
            },
        );
        actor.normal_attack = 924;
        actor
    }

    fn profile() -> EnemyAiProfile {
        EnemyAiProfile {
            role_id: 701,
            spells: [0; 4],
            uses_items: false,
        }
    }

    #[test]
    fn native_no_decision_keeps_a_ready_enemy_waiting_instead_of_always_attacking() {
        let mut actors = std::array::from_fn(|_| None);
        actors[0] = Some(actor());
        actors[5] = Some(actor());
        let mut rng = CombatRng::new(0);
        assert_eq!(
            choose_noncasting_physical(&actors, 5, &profile(), &mut rng),
            Ok(None)
        );
        assert_eq!(rng.draws(), 8);
        let mut rng = CombatRng::new(6);
        assert_eq!(
            choose_noncasting_physical(&actors, 5, &profile(), &mut rng),
            Ok(Some(0))
        );
        assert_eq!(rng.draws(), 2);
    }

    #[test]
    fn unsupported_decision_trees_do_not_mutate_rng_or_resources() {
        let mut actors = std::array::from_fn(|_| None);
        actors[0] = Some(actor());
        actors[5] = Some(actor());
        let mut rng = CombatRng::new(1);
        let before = rng;
        let mut caster = profile();
        caster.spells[0] = 210;
        assert_eq!(
            choose_noncasting_physical(&actors, 5, &caster, &mut rng),
            Err(AiError::UnsupportedCasting)
        );
        actors[5].as_mut().unwrap().vitals.current.hp = 101;
        assert_eq!(
            choose_noncasting_physical(&actors, 5, &profile(), &mut rng),
            Err(AiError::InvalidVitals)
        );
        assert_eq!(rng, before);
        assert_eq!(actors[0].as_ref().unwrap().vitals.current.hp, 100);
    }

    #[test]
    fn quarter_health_boundary_changes_priority_without_forcing_an_attack() {
        let mut actors = std::array::from_fn(|_| None);
        actors[0] = Some(actor());
        actors[5] = Some(actor());
        for (hp, draws) in [(50, 2), (25, 2), (24, 1), (1, 1)] {
            actors[5].as_mut().unwrap().vitals.current.hp = hp;
            let mut rng = CombatRng::new(6);
            assert_eq!(
                choose_noncasting_physical(&actors, 5, &profile(), &mut rng),
                Ok(Some(0))
            );
            assert_eq!(rng.draws(), draws);
        }
        let mut rng = CombatRng::new(0);
        assert_eq!(
            choose_noncasting_physical(&actors, 5, &profile(), &mut rng),
            Ok(None)
        );
    }

    #[test]
    fn dead_present_party_members_affect_the_area_attempt_but_are_not_targets() {
        let mut actors = std::array::from_fn(|_| None);
        let mut wounded = actor();
        wounded.vitals.current.hp = 49;
        actors[0] = Some(wounded);
        let mut dead = actor();
        dead.vitals.current.hp = 0;
        dead.vitals.enabled = false;
        actors[1] = Some(dead);
        actors[5] = Some(actor());
        let mut rng = CombatRng::new(6);
        assert_eq!(
            choose_noncasting_physical(&actors, 5, &profile(), &mut rng),
            Ok(Some(0))
        );
        assert_eq!(rng.draws(), 3);
        actors[5].as_mut().unwrap().vitals.current.hp = 24;
        let mut rng = CombatRng::new(6);
        assert_eq!(
            choose_noncasting_physical(&actors, 5, &profile(), &mut rng),
            Ok(Some(0))
        );
        assert_eq!(rng.draws(), 1);
    }

    #[test]
    #[ignore = "requires PAL3_AI_ORACLE"]
    fn original_noncasting_health_bands_match_proposals_and_rng() {
        use serde::Deserialize;
        #[derive(Deserialize)]
        struct Actor {
            hp: i32,
            max_hp: i32,
            enabled: bool,
        }
        #[derive(Deserialize)]
        struct ResultState {
            proposal: Option<[i32; 4]>,
            returned: u8,
            seed: u32,
            draws: u32,
        }
        #[derive(Deserialize)]
        struct Case {
            actors: [Option<Actor>; ACTOR_SLOTS],
            source: usize,
            seed: u32,
            result: ResultState,
        }
        #[derive(Deserialize)]
        struct Oracle {
            cases: Vec<Case>,
        }
        let oracle: Oracle = serde_json::from_slice(
            &std::fs::read(std::env::var("PAL3_AI_ORACLE").unwrap()).unwrap(),
        )
        .unwrap();
        assert!(!oracle.cases.is_empty());
        for (index, case) in oracle.cases.iter().enumerate() {
            let actors = std::array::from_fn(|slot| {
                case.actors[slot].as_ref().map(|a| {
                    let mut result = actor();
                    result.vitals.current.hp = a.hp;
                    result.vitals.maximum.hp = a.max_hp;
                    result.vitals.enabled = a.enabled;
                    result
                })
            });
            let mut rng = CombatRng::new(case.seed);
            let choice =
                choose_noncasting_physical(&actors, case.source, &profile(), &mut rng).unwrap();
            assert_eq!(
                choice.map(|target| [0x103, 0, case.source as i32, target as i32]),
                case.result.proposal,
                "native decision {index}"
            );
            assert_eq!(
                (choice.is_some() as u8, rng.state(), rng.draws()),
                (case.result.returned, case.result.seed, case.result.draws),
                "native RNG {index}"
            );
        }
        eprintln!(
            "Compared {} native non-casting AI decisions",
            oracle.cases.len()
        );
    }
}
