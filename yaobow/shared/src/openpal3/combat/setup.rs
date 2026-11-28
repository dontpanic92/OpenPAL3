use super::{
    CombatRng,
    ai::EnemyAiProfile,
    battle::{BattleState, PhysicalActor},
    timeline::{ACTOR_SLOTS, Opening},
};
use crate::openpal3::{
    loaders::stats::StatsData,
    states::{
        party::PartyState,
        stats::{Attributes, Resources, StatsState},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupError {
    UnknownRole(u32),
    UnknownEquipment(u32),
    UnsupportedConditions,
    UnsupportedBoss,
}

/// Scalar combat-entry state, before arena/presentation and death-animation setup.
/// Enemy IDs are already in runtime slot order; encounter selection is separate.
/// Unsupported equipment remains marked and cannot enter physical execution.
pub struct BattleSetup {
    pub battle: BattleState,
    pub enemy_profiles: [Option<EnemyAiProfile>; 6],
    pub formation_slots: [Option<u8>; ACTOR_SLOTS],
}

impl BattleSetup {
    pub fn new(
        party: &PartyState,
        stats: &StatsState,
        data: &StatsData,
        enemies: [Option<u32>; 6],
        opening: Opening,
        mut rng: CombatRng,
        money: i32,
    ) -> Result<Self, SetupError> {
        let mut actors = std::array::from_fn(|_| None);
        let mut formation_slots = [None; ACTOR_SLOTS];
        let mut enemy_profiles = std::array::from_fn(|_| None);
        for (index, id) in enemies.into_iter().enumerate() {
            let Some(id) = id else { continue };
            let definition = data.combat_role(id).ok_or(SetupError::UnknownRole(id))?;
            if matches!(id, 738 | 739) {
                return Err(SetupError::UnsupportedBoss);
            }
            if definition.initial_conditions.contains(&true) {
                return Err(SetupError::UnsupportedConditions);
            }
            let [
                hp,
                qi,
                mp,
                attack,
                defense,
                speed,
                luck,
                water,
                fire,
                wind,
                thunder,
                earth,
            ] = definition.attributes;
            let mut actor = PhysicalActor::new(
                definition.level,
                Attributes {
                    hp,
                    qi,
                    mp,
                    attack,
                    defense,
                    speed,
                    luck,
                    elements: [water, fire, wind, thunder, earth],
                },
                Resources { hp, qi, mp },
            );
            actor.normal_attack = definition.normal_attack;
            actor.qi_gain = definition.qi_gain;
            actor.reduced_pattern = matches!(actor.normal_attack, 916..=923 | 926..=928);
            actor.element_table = definition.attack_elements.map(|value| {
                let value = value as u32;
                (value << 16) | value
            });
            actors[5 + index] = Some(actor);
            formation_slots[5 + index] = Some(index as u8 + 1);
            enemy_profiles[index] = Some(EnemyAiProfile::from_role(definition));
        }
        // 0x4ed8c0 initializes weapon patterns for all five saved characters,
        // even absent ones, and falls back to the magic sword when unequipped.
        for role in 0..5 {
            let character = stats.character(role as i32).unwrap();
            let weapon_id = character.equipment()[5].unwrap_or(6603);
            let weapon = data
                .item(weapon_id)
                .ok_or(SetupError::UnknownEquipment(weapon_id))?;
            let mut reduced = match weapon.subtype {
                0 => {
                    if rng.next_u15() % 1024 >= 307 {
                        rng.next_u15();
                    }
                    true
                }
                1 | 3 | 4 | 7 => true,
                _ => false,
            };
            if role == 0 {
                reduced = true;
            }
            if !party.contains(role as i32) {
                continue;
            }
            if character.poisons().iter().any(|&value| value != 0) {
                return Err(SetupError::UnsupportedConditions);
            }
            let mut actor = PhysicalActor::new(
                character.level(),
                stats.attributes(role as i32, data).unwrap(),
                character.current(),
            );
            actor.reduced_pattern = reduced;
            actor.hit_divisor = weapon.hit_divisor.max(1);
            for (slot, id) in character.equipment().iter().enumerate() {
                let item = if slot == 5 {
                    Some(weapon)
                } else {
                    id.map(|id| data.item(id).ok_or(SetupError::UnknownEquipment(id)))
                        .transpose()?
                };
                let Some(item) = item else { continue };
                for (element, &flag) in item.elements.iter().enumerate() {
                    if slot == 5 {
                        actor.element_table[element] |= u32::from(flag) << 16;
                    } else {
                        actor.element_table[element] += u32::from(flag);
                    }
                }
                // 0x4edf10 applies passive percentages to non-weapon slots only.
                if slot != 5 {
                    actor.vitals.qi_discount_percent += f32::from(item.combat_percentages[0]);
                    actor.vitals.mp_discount_percent += f32::from(item.combat_percentages[1]);
                    actor.critical_bonus_percent += f32::from(item.combat_percentages[2]);
                }
                // These data must not be silently stripped to obtain a playable
                // physical-only roster: procs, regeneration and special sets
                // still require their native consumers and status lifecycle.
                actor.unsupported_equipment |= item.conditions.iter().any(|&value| value != 0)
                    || (20..=24).contains(&item.use_kind)
                    || (slot != 5
                        && (item.amounts[..3].iter().any(|&value| value != 0)
                            || item.combat_percentages[3] != 0))
                    || matches!(item.id, 80015 | 84018 | 83012 | 8153 | 8154 | 8110);
            }
            actors[role] = Some(actor);
            formation_slots[role] = party.formation_slot(role as i32);
        }
        Ok(Self {
            battle: BattleState::with_rng(actors, opening, rng, money),
            enemy_profiles,
            formation_slots,
        })
    }
}

/// Native 0x4ed170 ordinary-encounter permutation: 100 pairs of slot draws,
/// including empty slots and self-swaps. Scripted/fixed formations skip it.
pub fn shuffle_enemy_slots(enemies: &mut [Option<u32>; 6], rng: &mut CombatRng) {
    for _ in 0..100 {
        let left = (rng.next_u15() % 6) as usize;
        let right = (rng.next_u15() % 6) as usize;
        enemies.swap(left, right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::store_ext::StoreExt2;
    use serde::Deserialize;

    #[test]
    fn setup_rejects_unknown_data_and_unsupported_poison_without_changing_world_stats() {
        let data = crate::openpal3::loaders::stats::test_data();
        let party = PartyState::default();
        let mut stats = StatsState::default();
        let build = |stats: &StatsState, enemies| {
            BattleSetup::new(
                &party,
                stats,
                &data,
                enemies,
                Opening::Normal,
                CombatRng::new(6),
                123,
            )
        };
        assert!(matches!(
            build(&stats, [Some(999); 6]),
            Err(SetupError::UnknownRole(999))
        ));
        // The missing weapon belongs to an absent character, but native entry
        // still accesses it during the five-character pattern initialization.
        assert!(matches!(
            build(&stats, [None; 6]),
            Err(SetupError::UnknownEquipment(82101))
        ));
        stats.set_poison(0, 0, 1);
        let before = stats.clone();
        assert!(matches!(
            build(&stats, [None; 6]),
            Err(SetupError::UnsupportedConditions)
        ));
        assert_eq!(stats, before);
    }

    #[derive(Deserialize)]
    struct Character {
        level: i32,
        base: [i32; 12],
        current: [i32; 3],
        equipment: [Option<u32>; 6],
    }

    fn attributes(values: [i32; 12]) -> Attributes {
        let [
            hp,
            qi,
            mp,
            attack,
            defense,
            speed,
            luck,
            water,
            fire,
            wind,
            thunder,
            earth,
        ] = values;
        Attributes {
            hp,
            qi,
            mp,
            attack,
            defense,
            speed,
            luck,
            elements: [water, fire, wind, thunder, earth],
        }
    }

    fn states(members: u32, characters: &[Character; 5]) -> (PartyState, StatsState) {
        let party = serde_json::from_value(serde_json::json!({
            "members": std::array::from_fn::<_, 5, _>(|role| members & (1 << role) != 0),
            "formation_slots": [1, 2, 3, 4, 5]
        }))
        .unwrap();
        let mut value = serde_json::to_value(StatsState::default()).unwrap();
        for (role, character) in characters.iter().enumerate() {
            let saved = &mut value["characters"][role];
            saved["level"] = character.level.into();
            saved["base"] = serde_json::to_value(attributes(character.base)).unwrap();
            let [hp, qi, mp] = character.current;
            saved["current"] = serde_json::to_value(Resources { hp, qi, mp }).unwrap();
            saved["equipment"] = serde_json::to_value(character.equipment).unwrap();
        }
        (party, serde_json::from_value(value).unwrap())
    }

    #[test]
    #[ignore = "requires original assets and PAL3_SETUP_ORACLE"]
    fn original_scalar_entry_preserves_roster_resources_equipment_and_rng() {
        #[derive(Deserialize)]
        struct Actor {
            level: i32,
            attributes: [i32; 7],
            current: [i32; 3],
            enabled: bool,
            normal_attack: u32,
            qi_gain: i32,
            reduced: bool,
            elements: [u32; 5],
            percentages: [f32; 3],
            formation: u8,
            ready: i32,
        }
        #[derive(Deserialize)]
        struct ResultState {
            actors: [Option<Actor>; ACTOR_SLOTS],
            seed: u32,
            draws: u32,
        }
        #[derive(Deserialize)]
        struct Case {
            members: u32,
            enemies: [Option<u32>; 6],
            characters: [Character; 5],
            seed: u32,
            opening: u8,
            random_patterns: bool,
            result: ResultState,
        }
        #[derive(Deserialize)]
        struct Shuffle {
            enemies: [Option<u32>; 6],
            seed: u32,
            result: [Option<u32>; 6],
            next_seed: u32,
            draws: u32,
        }
        #[derive(Deserialize)]
        struct Oracle {
            cases: Vec<Case>,
            shuffle: Vec<Shuffle>,
        }
        let vfs = packfs::init_virtual_fs(std::env::var("PAL3_ASSET_PATH").unwrap(), None);
        let mut database = vfs
            .read_to_end("/basedata/basedata/cbdata/PAL3_Softstar.gdb")
            .unwrap();
        let combat = vfs
            .read_to_end_from_gbk("/basedata/basedata/datascript/combat.ini")
            .unwrap();
        let data = StatsData::load(&database, &combat).unwrap();
        let offset = u32::from_le_bytes(database[0x90..0x94].try_into().unwrap()) as usize;
        let count = u32::from_le_bytes(database[0x94..0x98].try_into().unwrap()) as usize;
        let record = database[offset..offset + count * 800]
            .chunks_exact_mut(800)
            .find(|record| u32::from_le_bytes(record[..4].try_into().unwrap()) == 82101)
            .unwrap();
        record[0x265] = 0;
        let random_data = StatsData::load(&database, &combat).unwrap();
        let oracle: Oracle = serde_json::from_slice(
            &std::fs::read(std::env::var("PAL3_SETUP_ORACLE").unwrap()).unwrap(),
        )
        .unwrap();
        assert!(!oracle.cases.is_empty() && !oracle.shuffle.is_empty());
        for (index, case) in oracle.cases.iter().enumerate() {
            let (party, stats) = states(case.members, &case.characters);
            let setup = BattleSetup::new(
                &party,
                &stats,
                if case.random_patterns {
                    &random_data
                } else {
                    &data
                },
                case.enemies,
                match case.opening {
                    1 => Opening::PartyReady,
                    2 => Opening::EnemiesReady,
                    _ => Opening::Normal,
                },
                CombatRng::new(case.seed),
                123,
            )
            .unwrap();
            for slot in 0..ACTOR_SLOTS {
                let expected = &case.result.actors[slot];
                let actual = setup.battle.actor(slot);
                assert_eq!(
                    actual.is_some(),
                    expected.is_some(),
                    "setup {index} slot {slot}"
                );
                let Some(expected) = expected else { continue };
                let actual = actual.unwrap();
                let a = actual.attributes;
                assert_eq!(
                    [a.hp, a.qi, a.mp, a.attack, a.defense, a.speed, a.luck],
                    expected.attributes,
                    "stats {index}:{slot}"
                );
                let c = actual.vitals.current;
                assert_eq!(
                    [c.hp, c.qi, c.mp],
                    expected.current,
                    "resources {index}:{slot}"
                );
                assert_eq!(
                    (
                        actual.level,
                        actual.vitals.enabled,
                        actual.normal_attack,
                        actual.qi_gain,
                        actual.reduced_pattern
                    ),
                    (
                        expected.level,
                        expected.enabled,
                        expected.normal_attack,
                        expected.qi_gain,
                        expected.reduced
                    ),
                    "actor {index}:{slot}"
                );
                assert_eq!(
                    actual.element_table, expected.elements,
                    "elements {index}:{slot}"
                );
                assert_eq!(
                    [
                        actual.vitals.qi_discount_percent,
                        actual.vitals.mp_discount_percent,
                        actual.critical_bonus_percent
                    ],
                    expected.percentages,
                    "percentages {index}:{slot}"
                );
                assert_eq!(setup.formation_slots[slot], Some(expected.formation));
                assert_eq!(
                    setup.battle.timeline().meter(slot).unwrap().ready,
                    expected.ready,
                    "meter {index}:{slot}"
                );
            }
            assert_eq!(
                (setup.battle.rng().state(), setup.battle.rng().draws()),
                (case.result.seed, case.result.draws),
                "setup RNG {index}"
            );
        }
        for case in &oracle.shuffle {
            let mut enemies = case.enemies;
            let mut rng = CombatRng::new(case.seed);
            shuffle_enemy_slots(&mut enemies, &mut rng);
            assert_eq!(enemies, case.result);
            assert_eq!((rng.state(), rng.draws()), (case.next_seed, case.draws));
        }
        eprintln!(
            "Compared {} native scalar battle setups and {} permutations",
            oracle.cases.len(),
            oracle.shuffle.len()
        );
    }
}
