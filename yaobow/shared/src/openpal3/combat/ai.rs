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
    UnsupportedHealthBand,
    UnsupportedStatusOrFormation,
    UnsupportedOnHitEffect,
    UnsupportedPresentation,
}

/// Native 0x4d7120 -> 0x4d71e0 for healthy, non-casting ordinary enemies.
/// No decision is a real native outcome; it does not consume the ready meter.
/// All actor presentations must be at rest. Other decision trees/capabilities
/// are rejected without advancing RNG.
pub fn choose_healthy_physical(
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
            || actor.vitals.current.hp <= 0
            || actor.vitals.current.hp > actor.vitals.maximum.hp
            || actor.vitals.current.hp < actor.vitals.maximum.hp / 2
        {
            return Err(AiError::UnsupportedHealthBand);
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
        if slot < 5 {
            // The 15% casting attempt still consumes its RNG draw when
            // casting is unavailable. The ordinary physical fallback follows.
            rng.next_u15();
            return Ok(Some(slot));
        }
        // With healthy caches and no magic/items, enemy-side candidates cannot
        // produce an action, but the two nested 30% magic attempts still draw.
        if rng.next_u15() % 1024 < 307 {
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
            choose_healthy_physical(&actors, 5, &profile(), &mut rng),
            Ok(None)
        );
        assert_eq!(rng.draws(), 8);
        let mut rng = CombatRng::new(6);
        assert_eq!(
            choose_healthy_physical(&actors, 5, &profile(), &mut rng),
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
            choose_healthy_physical(&actors, 5, &caster, &mut rng),
            Err(AiError::UnsupportedCasting)
        );
        actors[5].as_mut().unwrap().vitals.current.hp = 49;
        assert_eq!(
            choose_healthy_physical(&actors, 5, &profile(), &mut rng),
            Err(AiError::UnsupportedHealthBand)
        );
        assert_eq!(rng, before);
        assert_eq!(actors[0].as_ref().unwrap().vitals.current.hp, 100);
    }

    #[test]
    #[ignore = "requires PAL3_AI_ORACLE"]
    fn original_healthy_noncasting_decisions_match_proposals_and_rng() {
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
                choose_healthy_physical(&actors, case.source, &profile(), &mut rng).unwrap();
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
            "Compared {} native healthy non-casting AI decisions",
            oracle.cases.len()
        );
    }
}
