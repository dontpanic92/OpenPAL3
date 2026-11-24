use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use fileformats::pal3::gdb::{ItemStats, read_item_stats};
use ini::Ini;

pub struct StatsData {
    experience: [i32; 100],
    items: HashMap<u32, ItemStats>,
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
        for item in read_item_stats(database)? {
            items.entry(item.id).or_insert(item);
        }
        Ok(Self { experience, items })
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

    pub fn item(&self, id: u32) -> Option<&ItemStats> {
        self.items.get(&id)
    }
}

#[cfg(test)]
pub(crate) fn test_data() -> StatsData {
    let mut experience = std::array::from_fn(|i| i as i32 * 100);
    experience[99] = i32::MAX;
    StatsData {
        experience,
        items: HashMap::from([
            (
                82201,
                ItemStats {
                    id: 82201,
                    modes: [0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                    amounts: [5, 0, 99, 7, -2, -1, 0, 120, 0, -100, 0, 0],
                    elements: [false, false, false, true, false],
                },
            ),
            (
                99999,
                ItemStats {
                    id: 99999,
                    modes: [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                    amounts: [20, 5, -3, -2, 0, 2, 1, -2, 0, 4, 0, 0],
                    elements: [true, false, true, false, false],
                },
            ),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
