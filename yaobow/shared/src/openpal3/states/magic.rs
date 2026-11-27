use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::openpal3::loaders::stats::StatsData;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openpal3::{
        loaders::stats::test_data_with_magic, states::persistent_state::PersistentState,
    };
    use fileformats::pal3::gdb::MagicDefinition;

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
