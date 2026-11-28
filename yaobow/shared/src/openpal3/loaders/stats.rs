use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result, bail};
use fileformats::pal3::gdb::{
    ItemDefinition, MagicDefinition, RoleDefinition, read_items, read_magic, read_roles,
};
use ini::Ini;

pub struct StatsData {
    experience: [i32; 100],
    items: HashMap<u32, ItemDefinition>,
    magic: BTreeMap<u32, MagicDefinition>,
    combat_roles: BTreeMap<u32, RoleDefinition>,
}

impl StatsData {
    pub fn load(database: &[u8], combat_ini: &str) -> Result<Self> {
        let ini = Ini::load_from_str(combat_ini)?;
        let levels = ini
            .section(Some("Level"))
            .context("Missing PAL3 [Level] table")?;
        let mut experience = [i32::MAX; 100];
        for (index, threshold) in experience[..99].iter_mut().enumerate() {
            let key = (index + 1).to_string();
            *threshold = levels
                .get(&key)
                .with_context(|| format!("Missing PAL3 level {key}"))?
                .trim()
                .parse()
                .with_context(|| format!("Invalid PAL3 level {key} EXP"))?;
        }
        if experience[0] != 0 || experience.windows(2).any(|pair| pair[0] >= pair[1]) {
            bail!("Invalid PAL3 EXP threshold order");
        }
        let mut items = HashMap::new();
        for item in read_items(database)? {
            items.entry(item.id).or_insert(item);
        }
        let mut magic = BTreeMap::new();
        for definition in read_magic(database)? {
            magic.entry(definition.id).or_insert(definition);
        }
        let mut combat_roles = BTreeMap::new();
        for definition in read_roles(database)? {
            combat_roles.entry(definition.id).or_insert(definition);
        }
        Ok(Self {
            experience,
            items,
            magic,
            combat_roles,
        })
    }

    pub fn experience_threshold(&self, level: i32) -> i32 {
        self.experience[(level.clamp(1, 100) - 1) as usize]
    }

    pub fn level_for_experience(&self, experience: i32) -> i32 {
        self.experience
            .iter()
            .position(|&threshold| threshold as u32 > experience as u32)
            .unwrap_or(100) as i32
    }

    pub fn item(&self, id: u32) -> Option<&ItemDefinition> {
        self.items.get(&id)
    }

    pub fn items(&self) -> impl Iterator<Item = &ItemDefinition> {
        self.items.values()
    }

    pub fn magic(&self, id: u32) -> Option<&MagicDefinition> {
        self.magic.get(&id)
    }

    pub fn magics(&self) -> impl Iterator<Item = &MagicDefinition> {
        self.magic.values()
    }

    pub fn combat_role(&self, id: u32) -> Option<&RoleDefinition> {
        self.combat_roles.get(&id)
    }
}

#[cfg(test)]
pub(crate) fn test_data_with_items(items: &[ItemDefinition]) -> StatsData {
    let mut data = test_data();
    for item in items {
        data.items.insert(item.id, item.clone());
    }
    data
}

#[cfg(test)]
pub(crate) fn test_data() -> StatsData {
    let mut experience = std::array::from_fn(|i| i as i32 * 100);
    experience[99] = i32::MAX;
    let mut items = HashMap::from([
        (
            82201,
            ItemDefinition {
                id: 82201,
                modes: [0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                amounts: [5, 0, 99, 7, -2, -1, 0, 120, 0, -100, 0, 0],
                elements: [false, false, false, true, false],
                ..Default::default()
            },
        ),
        (
            99999,
            ItemDefinition {
                id: 99999,
                modes: [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                amounts: [20, 5, -3, -2, 0, 2, 1, -2, 0, 4, 0, 0],
                elements: [true, false, true, false, false],
                ..Default::default()
            },
        ),
    ]);
    for id in [
        42, 6008, 6119, 6120, 6122, 6123, 6124, 6125, 6427, 6601, 6602, 6603, 8146, 8147, 8149,
        8150, 8151, 8152,
    ] {
        items.insert(
            id,
            ItemDefinition {
                id,
                category: if [6601, 6602, 6603].contains(&id) {
                    11
                } else {
                    2
                },
                usable_by: [true; 5],
                ..Default::default()
            },
        );
    }
    StatsData {
        experience,
        items,
        magic: BTreeMap::new(),
        combat_roles: BTreeMap::new(),
    }
}

#[cfg(test)]
pub(crate) fn test_data_with_magic(definitions: &[MagicDefinition]) -> StatsData {
    let mut data = test_data();
    for definition in definitions {
        data.magic.insert(definition.id, definition.clone());
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original game assets via PAL3_ASSET_PATH"]
    fn original_magic_catalog_preserves_names_roles_and_spell_book_mapping() {
        use common::store_ext::StoreExt2;
        let path = std::env::var("PAL3_ASSET_PATH").expect("PAL3_ASSET_PATH");
        let vfs = packfs::init_virtual_fs(path, None);
        let database = vfs
            .read_to_end("/basedata/basedata/cbdata/PAL3_Softstar.gdb")
            .unwrap();
        let combat = vfs
            .read_to_end_from_gbk("/basedata/basedata/datascript/combat.ini")
            .unwrap();
        let data = StatsData::load(&database, &combat).unwrap();
        assert_eq!(data.magics().count(), 187);
        assert_eq!(
            data.magics()
                .filter(|m| matches!(m.kind, 1..=3) && m.usable_by.contains(&true))
                .count(),
            33
        );
        assert_eq!(
            data.magics()
                .filter(|m| m.kind == 0 && m.usable_by.contains(&true))
                .count(),
            26
        );
        for (book, spell, excluded_role, element) in [
            (6409, 306, 2, 1),
            (6410, 311, 0, 3),
            (6411, 301, 1, 2),
            (6412, 316, 4, 0),
            (6413, 321, 3, 4),
        ] {
            let magic = data.magic(spell).unwrap();
            assert_eq!(magic.kind, 2);
            assert!(!magic.name.is_empty() && !magic.description.is_empty());
            assert_eq!(magic.elements, std::array::from_fn(|i| i == element));
            assert_eq!(magic.usable_by, std::array::from_fn(|i| i != excluded_role));
            let item = data.item(book).unwrap();
            assert_eq!(item.use_kind, (book - 6402) as u8);
            assert_eq!(item.usable_by, [false; 5]);
        }
        for (role, skill) in [
            (0, 4101),
            (0, 4103),
            (0, 4104),
            (0, 4106),
            (1, 4201),
            (2, 4305),
            (3, 4402),
        ] {
            let magic = data.magic(skill).unwrap();
            assert_eq!(magic.kind, 0);
            assert!(magic.usable_by[role]);
        }
    }

    #[test]
    fn loads_exp_thresholds_and_preserves_level_100_sentinel() {
        let mut database = vec![0; 0xa0];
        let magic = b"__PAL(Softstar_Ltd: Cool_J)_GameDataBase_OutFile__";
        database[..magic.len()].copy_from_slice(magic);
        database[0x90..0x94].copy_from_slice(&0xa0u32.to_le_bytes());
        let ini = format!(
            "[Level]\n{}",
            (1..=99)
                .map(|level| format!("{level}={}\n", (level - 1) * 100))
                .collect::<String>()
        );
        let data = StatsData::load(&database, &ini).unwrap();
        assert_eq!(data.experience_threshold(-1), 0);
        assert_eq!(data.experience_threshold(99), 9800);
        assert_eq!(data.experience_threshold(100), i32::MAX);
        assert_eq!(data.experience_threshold(105), i32::MAX);
        for (exp, level) in [(0, 1), (99, 1), (100, 2), (9800, 99), (i32::MAX, 100)] {
            assert_eq!(data.level_for_experience(exp), level);
        }
        assert!(StatsData::load(&database, "[Level]\n1=0\n").is_err());
        assert!(StatsData::load(&database, &ini.replace("2=100", "2=0")).is_err());
    }
}
