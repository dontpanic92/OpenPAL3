use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::openpal3::loaders::stats::StatsData;

// Native script-level setter 0x40ff70 uses this schedule, not the battle
// metadata. In particular, it does not auto-grant 4106 at level 58.
const SCRIPT_LEVEL_SKILLS: [&[(u32, i32)]; 5] = [
    &[(4102, 4), (4105, 44)],
    &[(4202, 15), (4203, 20), (4204, 55), (4205, 50)],
    &[(4301, 3), (4302, 10), (4303, 15), (4304, 20)],
    &[(4401, 4), (4403, 13), (4404, 35), (4405, 58)],
    &[(4501, 8), (4502, 24), (4503, 38), (4504, 50), (4505, 64)],
];

#[derive(Default, Serialize, Deserialize)]
pub struct SpellProgress {
    pub level: u8,
    pub uses: u8,
}

#[derive(Serialize, Deserialize)]
pub struct MagicState {
    learned: [BTreeMap<u32, SpellProgress>; 5],
}

impl Default for MagicState {
    fn default() -> Self {
        let mut learned: [BTreeMap<u32, SpellProgress>; 5] = Default::default();
        // Native new-game initialization grants only Jingtian's wind spell.
        learned[0].insert(301, SpellProgress::default());
        Self { learned }
    }
}

impl MagicState {
    pub fn progress(&self, role: i32, id: u32) -> Option<&SpellProgress> {
        usize::try_from(role)
            .ok()
            .and_then(|role| self.learned.get(role))
            .and_then(|spells| spells.get(&id))
    }

    pub fn learn(&mut self, role: i32, id: u32, data: &StatsData) -> bool {
        let Some(definition) = data.magic(id) else {
            return false;
        };
        let Some(role) = usize::try_from(role).ok().filter(|&role| role < 5) else {
            return false;
        };
        if !matches!(definition.kind, 0..=3) || !definition.usable_by[role] {
            return false;
        }
        self.learned[role].entry(id).or_default();
        true
    }

    fn learn_new(&mut self, role: i32, id: u32, data: &StatsData, learned: &mut Vec<u32>) {
        if self.progress(role, id).is_none() && self.learn(role, id, data) {
            learned.push(id);
        }
    }

    pub fn learn_from_script_level(&mut self, role: i32, level: i32, data: &StatsData) -> Vec<u32> {
        let Some(schedule) = usize::try_from(role)
            .ok()
            .and_then(|r| SCRIPT_LEVEL_SKILLS.get(r))
        else {
            return vec![];
        };
        let mut learned = vec![];
        for &(id, required) in *schedule {
            if level >= required {
                self.learn_new(role, id, data, &mut learned);
            }
        }
        learned
    }

    pub fn learn_after_level_up(&mut self, role: i32, level: i32, data: &StatsData) -> Vec<u32> {
        let Some(index) = usize::try_from(role).ok().filter(|&r| r < 5) else {
            return vec![];
        };
        let mut learned = vec![];
        for definition in data.magics() {
            if definition.kind == 0
                && definition.usable_by[index]
                && definition.learning_level != 0
                && level >= i32::from(definition.learning_level)
            {
                self.learn_new(role, definition.id, data, &mut learned);
            }
        }
        learned.extend(self.refresh_spell_learning(role, level, data));
        learned
    }

    /// Native 0x4fae8d..0x4fb328 evaluates recorded uses and prerequisites.
    /// It does not cast spells or manufacture usage credit.
    pub fn refresh_spell_learning(&mut self, role: i32, level: i32, data: &StatsData) -> Vec<u32> {
        let Some(index) = usize::try_from(role).ok().filter(|&r| r < 5) else {
            return vec![];
        };
        let in_catalog = |definition: &fileformats::pal3::gdb::MagicDefinition| {
            matches!(definition.kind, 1..=3)
                && definition.usable_by[index]
                && definition.elements.iter().any(|&element| element)
        };
        let mut catalog: Vec<_> = data.magics().filter(|d| in_catalog(d)).collect();
        catalog.sort_by_key(|d| {
            let group = if d.elements.iter().filter(|&&element| element).count() > 1 {
                5
            } else {
                [0, 1, 3, 2, 4]
                    .iter()
                    .position(|&element| d.elements[element])
                    .unwrap()
            };
            (group, d.id)
        });
        let available = |id| data.magic(id).is_some_and(&in_catalog);
        // The original successor path ignores a failed catalog lookup and can
        // use unset slots (e.g. role 0's 313 -> unavailable 314). Reject it
        // rather than granting an unrelated spell from undefined slot data.
        let mut learned = vec![];
        loop {
            for definition in &catalog {
                let Some(progress) = self.learned[index].get_mut(&definition.id) else {
                    continue;
                };
                if progress.level < 3
                    && progress.uses >= definition.proficiency_uses[progress.level as usize]
                {
                    if let Some(rank) = (0..3)
                        .rev()
                        .find(|&rank| progress.uses >= definition.proficiency_uses[rank])
                    {
                        progress.level = rank as u8 + 1;
                    }
                }
            }
            for definition in &catalog {
                let Some(rank) = self
                    .progress(role, definition.id)
                    .map(|p| u16::from(p.level) + 1)
                else {
                    continue;
                };
                for recipe in &definition.mixed_recipes {
                    if recipe.spell != 0
                        && recipe.other_spell != 0
                        && rank >= u16::from(recipe.source_rank)
                        && level >= i32::from(recipe.character_level)
                        && available(recipe.other_spell)
                        && available(recipe.spell)
                        && self
                            .progress(role, recipe.other_spell)
                            .is_some_and(|p| u16::from(p.level) + 1 >= u16::from(recipe.other_rank))
                    {
                        self.learn_new(role, recipe.spell, data, &mut learned);
                    }
                }
            }
            let mut successor_learned = false;
            for definition in &catalog {
                let Some(rank) = self
                    .progress(role, definition.id)
                    .map(|p| u16::from(p.level) + 1)
                else {
                    continue;
                };
                if definition.successor != 0
                    && available(definition.successor)
                    && rank >= u16::from(definition.successor_rank)
                    && level >= i32::from(definition.learning_level)
                    && self.progress(role, definition.successor).is_none()
                {
                    self.learn_new(role, definition.successor, data, &mut learned);
                    successor_learned = true;
                }
            }
            // The original repeats only after a newly learned successor;
            // mixed-spell learning alone does not restart the pass.
            if !successor_learned {
                break;
            }
        }
        learned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openpal3::{
        loaders::stats::test_data_with_magic, states::persistent_state::PersistentState,
    };
    use fileformats::pal3::gdb::MagicDefinition;

    #[test]
    #[ignore = "requires original PAL3 assets and PAL3_AUTO_MAGIC_ORACLE"]
    fn original_automatic_learning_matches_native_rules() {
        use common::store_ext::StoreExt2;
        #[derive(Deserialize)]
        struct Case {
            mode: String,
            role: i32,
            level: i32,
            learned: BTreeMap<u32, [u8; 2]>,
            result: BTreeMap<u32, [u8; 2]>,
            #[serde(default)]
            unsafe_successor: bool,
        }
        #[derive(Deserialize)]
        struct Oracle {
            cases: Vec<Case>,
        }
        let path = std::env::var("PAL3_ASSET_PATH").unwrap();
        let vfs = packfs::init_virtual_fs(path, None);
        let data = StatsData::load(
            &vfs.read_to_end("/basedata/basedata/cbdata/PAL3_Softstar.gdb")
                .unwrap(),
            &vfs.read_to_end_from_gbk("/basedata/basedata/datascript/combat.ini")
                .unwrap(),
        )
        .unwrap();
        let oracle: Oracle = serde_json::from_slice(
            &std::fs::read(std::env::var("PAL3_AUTO_MAGIC_ORACLE").unwrap()).unwrap(),
        )
        .unwrap();
        let mut compared = 0;
        let mut undefined = 0;
        for (index, case) in oracle.cases.into_iter().enumerate() {
            if case.unsafe_successor {
                undefined += 1;
                continue;
            }
            compared += 1;
            let mut state = MagicState {
                learned: Default::default(),
            };
            for (&id, &[level, uses]) in &case.learned {
                assert!(state.learn(case.role, id, &data));
                *state.learned[case.role as usize].get_mut(&id).unwrap() =
                    SpellProgress { level, uses };
            }
            match case.mode.as_str() {
                "script" => {
                    state.learn_from_script_level(case.role, case.level, &data);
                }
                "level_up" => {
                    state.learn_after_level_up(case.role, case.level, &data);
                }
                "spells" => {
                    state.refresh_spell_learning(case.role, case.level, &data);
                }
                _ => panic!("Unknown original oracle mode"),
            }
            let actual: BTreeMap<_, _> = state.learned[case.role as usize]
                .iter()
                .map(|(&id, p)| (id, [p.level, p.uses]))
                .collect();
            assert_eq!(
                actual, case.result,
                "original case {index}, {}, role {}, level {}, seed {:?}",
                case.mode, case.role, case.level, case.learned
            );
        }
        assert!(compared > 0);
        eprintln!(
            "Compared {compared} defined original cases; {undefined} original cases have undefined successor slots"
        );
    }

    fn learning_data() -> StatsData {
        use fileformats::pal3::gdb::MagicRecipe;
        let starter = MagicDefinition {
            id: 301,
            kind: 2,
            elements: [false, false, true, false, false],
            usable_by: [true, false, false, false, false],
            proficiency_uses: [15, 30, 45],
            learning_level: 4,
            successor_rank: 2,
            successor: 302,
            ..Default::default()
        };
        let mut warm = MagicDefinition {
            id: 302,
            proficiency_uses: [20, 35, 45],
            learning_level: 5,
            successor: 303,
            ..starter.clone()
        };
        warm.mixed_recipes[0] = MagicRecipe {
            spell: 326,
            other_spell: 316,
            source_rank: 3,
            other_rank: 2,
            character_level: 35,
        };
        let mut definitions = vec![
            starter.clone(),
            warm,
            MagicDefinition {
                id: 303,
                successor: 0,
                ..starter.clone()
            },
            MagicDefinition {
                id: 316,
                elements: [true, false, false, false, false],
                successor: 0,
                ..starter.clone()
            },
            MagicDefinition {
                id: 326,
                elements: [true, false, true, false, false],
                proficiency_uses: [5, 10, 15],
                successor: 0,
                ..starter.clone()
            },
            MagicDefinition {
                id: 313,
                elements: [false, false, false, true, false],
                learning_level: 25,
                successor_rank: 3,
                successor: 314,
                ..starter.clone()
            },
            MagicDefinition {
                id: 314,
                elements: [false, false, false, true, false],
                usable_by: [false, false, true, false, false],
                successor: 0,
                ..starter
            },
        ];
        for (id, learning_level, role) in [
            (4101, 0, 0),
            (4102, 4, 0),
            (4105, 44, 0),
            (4106, 58, 0),
            (4202, 15, 1),
        ] {
            definitions.push(MagicDefinition {
                id,
                kind: 0,
                learning_level,
                usable_by: std::array::from_fn(|i| i == role),
                ..Default::default()
            });
        }
        test_data_with_magic(&definitions)
    }

    #[test]
    fn level_learning_keeps_script_schedule_distinct_from_ordinary_level_up() {
        let data = learning_data();
        let mut ordinary = MagicState::default();
        assert!(ordinary.learn_after_level_up(0, 3, &data).is_empty());
        assert_eq!(ordinary.learn_after_level_up(0, 4, &data), [4102]);
        assert!(ordinary.progress(0, 4101).is_none());
        assert_eq!(ordinary.learn_after_level_up(0, 58, &data), [4105, 4106]);
        assert!(ordinary.learn_after_level_up(0, 99, &data).is_empty());
        let mut script = MagicState::default();
        assert_eq!(script.learn_from_script_level(0, 58, &data), [4102, 4105]);
        assert!(script.progress(0, 4106).is_none());
        assert!(script.progress(0, 4202).is_none());
        let before = serde_json::to_string(&script).unwrap();
        assert!(script.learn_from_script_level(-1, 99, &data).is_empty());
        assert!(script.learn_after_level_up(5, 99, &data).is_empty());
        assert_eq!(serde_json::to_string(&script).unwrap(), before);
    }

    #[test]
    fn elemental_learning_requires_proficiency_and_level_without_creating_usage_credit() {
        let data = learning_data();
        let mut magic = MagicState::default();
        assert!(magic.refresh_spell_learning(0, 99, &data).is_empty());
        magic.learned[0].get_mut(&301).unwrap().uses = 14;
        assert!(magic.refresh_spell_learning(0, 4, &data).is_empty());
        magic.learned[0].get_mut(&301).unwrap().uses = 15;
        assert!(magic.refresh_spell_learning(0, 3, &data).is_empty());
        assert_eq!(magic.refresh_spell_learning(0, 4, &data), [302]);
        let progress = magic.progress(0, 301).unwrap();
        assert_eq!((progress.level, progress.uses), (1, 15));
        *magic.learned[0].get_mut(&302).unwrap() = SpellProgress { level: 2, uses: 19 };
        assert!(magic.refresh_spell_learning(0, 4, &data).is_empty());
        assert_eq!(magic.refresh_spell_learning(0, 5, &data), [303]);
        let progress = magic.progress(0, 302).unwrap();
        assert_eq!((progress.level, progress.uses), (2, 19));
        assert!(magic.refresh_spell_learning(0, 99, &data).is_empty());
    }

    #[test]
    fn mixed_learning_checks_both_owned_spells_ranks_and_character_level() {
        let data = learning_data();
        let mut magic = MagicState::default();
        magic.learn(0, 302, &data);
        magic.learn(0, 316, &data);
        magic.learned[0].get_mut(&302).unwrap().level = 2;
        assert!(
            magic
                .refresh_spell_learning(0, 35, &data)
                .iter()
                .all(|&id| id != 326)
        );
        magic.learned[0].get_mut(&316).unwrap().level = 1;
        assert!(
            magic
                .refresh_spell_learning(0, 34, &data)
                .iter()
                .all(|&id| id != 326)
        );
        assert_eq!(magic.refresh_spell_learning(0, 35, &data), [326]);
        *magic.learned[0].get_mut(&326).unwrap() = SpellProgress { level: 2, uses: 7 };
        assert!(magic.refresh_spell_learning(0, 99, &data).is_empty());
        let progress = magic.progress(0, 326).unwrap();
        assert_eq!((progress.level, progress.uses), (2, 7));
    }

    #[test]
    fn unavailable_successor_never_grants_an_unrelated_catalog_slot() {
        let data = learning_data();
        let mut magic = MagicState::default();
        magic.learn(0, 313, &data);
        *magic.learned[0].get_mut(&313).unwrap() = SpellProgress {
            level: 3,
            uses: 255,
        };
        assert!(magic.refresh_spell_learning(0, 99, &data).is_empty());
        assert!(magic.progress(0, 314).is_none());
        assert!(magic.progress(0, 316).is_none());
    }

    #[test]
    fn ordinary_level_hooks_preserve_injuries_exp_and_saved_learning() {
        use crate::openpal3::states::stats::Resource;
        let data = learning_data();
        let mut state = PersistentState::new("test".into());
        state.stats_mut().set_resource(0, Resource::Hp, 8, &data);
        state.stats_mut().set_resource(0, Resource::Mp, 3, &data);
        for _ in 0..3 {
            assert!(state.advance_level(0, &data));
        }
        assert!(state.magic().progress(0, 4102).is_some());
        state.magic_mut().learned[0].get_mut(&301).unwrap().uses = 15;
        assert!(state.advance_level(0, &data));
        assert!(state.magic().progress(0, 302).is_some());
        let character = state.stats().character(0).unwrap();
        assert_eq!(
            (
                character.experience(),
                character.current().hp,
                character.current().mp
            ),
            (0, 8, 3)
        );
        let restored: PersistentState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_string(state.magic()).unwrap(),
            serde_json::to_string(restored.magic()).unwrap()
        );
        assert!(!state.advance_level(-1, &data));
    }

    #[test]
    fn averaging_learns_only_after_native_setter_runs_even_without_a_level_increase() {
        let data = learning_data();
        let mut state = PersistentState::new("test".into());
        while state.stats().character(0).unwrap().level() < 60 {
            state.stats_mut().advance_level(0, &data);
        }
        *state.magic_mut().learned[0].get_mut(&301).unwrap() = SpellProgress { level: 1, uses: 15 };
        assert!(state.average_level(0, 0, &data));
        assert!(
            state.magic().progress(0, 4102).is_none(),
            "No other party members means no setter"
        );
        state.party_mut().set_in_team(1, true);
        assert!(state.average_level(0, 0, &data));
        assert_eq!(state.stats().character(0).unwrap().level(), 60);
        assert!(state.magic().progress(0, 4102).is_some());
        assert!(state.magic().progress(0, 4105).is_some());
        assert!(state.magic().progress(0, 4106).is_none());
        assert!(
            state.magic().progress(0, 302).is_none(),
            "Script-level path does not run elemental learning"
        );
    }

    fn data() -> StatsData {
        test_data_with_magic(&[
            MagicDefinition {
                id: 306,
                kind: 2,
                elements: [false, true, false, false, false],
                usable_by: [true, true, false, true, true],
                ..Default::default()
            },
            MagicDefinition {
                id: 4101,
                kind: 0,
                usable_by: [true, false, false, false, false],
                ..Default::default()
            },
            MagicDefinition {
                id: 2209,
                kind: 4,
                usable_by: [true; 5],
                ..Default::default()
            },
        ])
    }

    #[test]
    fn script_grants_validate_catalog_roles_and_preserve_duplicate_proficiency() {
        let data = data();
        let mut magic = MagicState::default();
        assert!(magic.learn(0, 306, &data));
        assert!(magic.learn(0, 4101, &data));
        *magic.learned[0].get_mut(&306).unwrap() = SpellProgress { level: 3, uses: 27 };
        assert!(magic.learn(0, 306, &data));
        let progress = magic.progress(0, 306).unwrap();
        assert_eq!((progress.level, progress.uses), (3, 27));
        let before = serde_json::to_string(&magic).unwrap();
        for (role, id) in [
            (-1, 306),
            (5, 306),
            (2, 306),
            (1, 4101),
            (0, 999),
            (0, 2209),
        ] {
            assert!(!magic.learn(role, id, &data));
        }
        assert!(magic.progress(-1, 306).is_none());
        assert!(magic.progress(5, 306).is_none());
        assert_eq!(serde_json::to_string(&magic).unwrap(), before);
    }

    #[test]
    fn saved_skill_ownership_roundtrips_and_old_saves_get_only_native_baseline() {
        let mut state = PersistentState::new("test".into());
        assert!(state.magic_mut().learn(1, 306, &data()));
        assert!(state.magic_mut().learn(0, 4101, &data()));
        *state.magic_mut().learned[1].get_mut(&306).unwrap() = SpellProgress { level: 2, uses: 19 };
        let mut json = serde_json::to_value(&state).unwrap();
        let restored: PersistentState = serde_json::from_value(json.clone()).unwrap();
        assert!(restored.magic().progress(0, 4101).is_some());
        let progress = restored.magic().progress(1, 306).unwrap();
        assert_eq!((progress.level, progress.uses), (2, 19));
        json.as_object_mut().unwrap().remove("magic");
        let restored: PersistentState = serde_json::from_value(json).unwrap();
        assert!(restored.magic().progress(0, 301).is_some());
        assert!(restored.magic().progress(0, 4101).is_none());
        assert!(restored.magic().progress(1, 306).is_none());
        for role in 1..5 {
            assert!(restored.magic().learned[role].is_empty());
        }
    }
}
