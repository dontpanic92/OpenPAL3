use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result, bail, ensure};
use common::store_ext::StoreExt2;
use fileformats::pal4::database::{Database, Table, Value};
use mini_fs::MiniFs;
use serde::{Deserialize, Serialize};

use super::states::persistent_state::PLAYER_COUNT;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CharacterStats {
    pub integers: BTreeMap<String, i32>,
    pub rates: BTreeMap<String, f32>,
}

impl CharacterStats {
    pub fn integer(&self, name: &str) -> i32 {
        self.integers.get(name).copied().unwrap_or(0)
    }

    pub fn add(&mut self, other: &Self) {
        for (name, value) in &other.integers {
            let entry = self.integers.entry(name.clone()).or_default();
            *entry = entry.saturating_add(*value);
        }
        for (name, value) in &other.rates {
            *self.rates.entry(name.clone()).or_default() += value;
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoleDefinition {
    pub id: i32,
    pub name: String,
    pub model: String,
    pub stats: CharacterStats,
}

#[derive(Debug, Clone)]
pub struct LevelDefinition {
    pub experience: i32,
    pub stats: CharacterStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipmentType {
    Sword,
    DualSwords,
    Zither,
    Head,
    Body,
    Feet,
    Accessory,
}

impl EquipmentType {
    fn read(value: i32) -> Result<Self> {
        Ok(match value {
            0 => Self::Sword,
            1 => Self::DualSwords,
            2 => Self::Zither,
            3 => Self::Head,
            4 => Self::Body,
            5 => Self::Feet,
            6 => Self::Accessory,
            _ => bail!("unknown PAL4 equipment type {value}"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EquipmentSlot {
    Weapon,
    Head,
    Body,
    Feet,
    AccessoryOne,
    AccessoryTwo,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerEquipment {
    pub weapon: Option<i32>,
    pub head: Option<i32>,
    pub body: Option<i32>,
    pub feet: Option<i32>,
    pub accessories: [Option<i32>; 2],
}

impl PlayerEquipment {
    pub fn ids(&self) -> impl Iterator<Item = i32> + '_ {
        [
            self.weapon,
            self.head,
            self.body,
            self.feet,
            self.accessories[0],
            self.accessories[1],
        ]
        .into_iter()
        .flatten()
    }

    pub fn script_slot(&self, kind: EquipmentType) -> EquipmentSlot {
        match kind {
            EquipmentType::Sword | EquipmentType::DualSwords | EquipmentType::Zither => {
                EquipmentSlot::Weapon
            }
            EquipmentType::Head => EquipmentSlot::Head,
            EquipmentType::Body => EquipmentSlot::Body,
            EquipmentType::Feet => EquipmentSlot::Feet,
            EquipmentType::Accessory if self.accessories[0].is_none() => {
                EquipmentSlot::AccessoryOne
            }
            EquipmentType::Accessory if self.accessories[1].is_none() => {
                EquipmentSlot::AccessoryTwo
            }
            EquipmentType::Accessory => EquipmentSlot::AccessoryOne,
        }
    }

    pub fn at_mut(&mut self, slot: EquipmentSlot) -> &mut Option<i32> {
        match slot {
            EquipmentSlot::Weapon => &mut self.weapon,
            EquipmentSlot::Head => &mut self.head,
            EquipmentSlot::Body => &mut self.body,
            EquipmentSlot::Feet => &mut self.feet,
            EquipmentSlot::AccessoryOne => &mut self.accessories[0],
            EquipmentSlot::AccessoryTwo => &mut self.accessories[1],
        }
    }
}

#[derive(Debug, Clone)]
pub struct EquipmentDefinition {
    pub id: i32,
    pub kind: EquipmentType,
    pub name: String,
    pub description: String,
    pub model: String,
    pub texture: String,
    pub level_limits: [i32; PLAYER_COUNT],
    pub price: i32,
    pub skill_id: i32,
    pub stats: CharacterStats,
}

#[derive(Debug, Clone)]
pub struct PropertyDefinition {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub detail: String,
    pub category: i32,
    pub target: i32,
    pub model: String,
    pub texture: String,
    pub usable_by: [bool; PLAYER_COUNT],
    pub usable_in_menu: bool,
    pub droppable: bool,
    pub price: i32,
    pub skill_id: i32,
}

#[derive(Debug, Clone)]
pub struct SkillDefinition {
    pub kind: i32,
    pub trigger: i32,
    pub duration: i32,
    pub function: i32,
    pub parameters: [f32; 9],
    pub attached: i32,
    pub mutex: i32,
    pub status: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct RecoveryEffect {
    pub hp: bool,
    pub amount: f32,
    pub proportional: bool,
}

#[derive(Debug, Default)]
pub struct GameplayCatalog {
    pub roles: BTreeMap<usize, RoleDefinition>,
    pub levels: HashMap<(usize, i32), LevelDefinition>,
    pub equipment: BTreeMap<i32, EquipmentDefinition>,
    pub properties: BTreeMap<i32, PropertyDefinition>,
    pub skills: BTreeMap<i32, SkillDefinition>,
    pub scene_names: HashMap<(String, String), String>,
    pub story_descriptions: BTreeMap<i32, (String, String)>,
}

impl GameplayCatalog {
    pub fn is_portable_save_item(&self, id: i32) -> bool {
        // The native inventory dispatcher special-cases this item, not function 9120.
        let Some(item) = self.properties.get(&id) else {
            return false;
        };
        let Some(skill) = self.skills.get(&item.skill_id) else {
            return false;
        };
        id == 3182
            && item.usable_in_menu
            && item.category == 2
            && item.target == 0
            && skill.kind == 2
            && skill.trigger == 2
            && skill.duration == 0
            && skill.function == 9120
            && skill.parameters == [0.0; 9]
            && skill.attached == 0
            && skill.mutex == 0
            && skill.status == 0
    }

    pub fn recovery_effects(&self, mut id: i32) -> Option<Vec<RecoveryEffect>> {
        let mut visited = Vec::new();
        let mut effects = Vec::new();
        while id != 0 {
            if visited.contains(&id) || visited.len() >= 16 {
                return None;
            }
            visited.push(id);
            let skill = self.skills.get(&id)?;
            if skill.trigger != 2 || skill.duration != 0 || skill.mutex != 0 || skill.status != 0 {
                return None;
            }
            if skill.function != 9132 {
                if skill.function != 9118 || ![0, 2, 3].contains(&skill.kind) {
                    return None;
                }
                let p = skill.parameters;
                if p[0] != 1.0 || p[1] != 0.0 || p[2] != 0.0 {
                    return None;
                }
                for start in [3, 6] {
                    if p[start] == 0.0 {
                        continue;
                    }
                    if ![2.0, 6.0].contains(&p[start])
                        || p[start + 1] <= 0.0
                        || ![0.0, 4.0].contains(&p[start + 2])
                    {
                        return None;
                    }
                    effects.push(RecoveryEffect {
                        hp: p[start] == 2.0,
                        amount: p[start + 1],
                        proportional: p[start + 2] == 4.0,
                    });
                }
            }
            id = skill.attached;
        }
        if effects.is_empty() {
            None
        } else {
            Some(effects)
        }
    }

    pub fn can_equip(&self, slot: usize, level: i32, id: i32) -> bool {
        self.equipment
            .get(&id)
            .and_then(|equipment| equipment.level_limits.get(slot))
            .is_some_and(|minimum| *minimum > 0 && level >= *minimum)
    }

    pub fn load(vfs: &MiniFs) -> Result<Self> {
        let bytes = vfs
            .read_to_end("/gamedata/database/Pal4db.db")
            .context("reading PAL4 gameplay database")?;
        Self::from_database(&Database::read(&bytes)?)
    }

    pub fn from_database(database: &Database) -> Result<Self> {
        let mut catalog = Self::default();
        let table = required_table(database, "Role")?;
        for values in &table.rows {
            let row = Row { table, values };
            let id = row.integer("ROLE_ID")?;
            let slot = role_slot(id)?;
            let role = RoleDefinition {
                id,
                name: row.text("RA_NAME")?,
                model: row.text("RA_MODEL")?,
                stats: row.stats()?,
            };
            ensure!(
                catalog.roles.insert(slot, role).is_none(),
                "duplicate role {id}"
            );
        }
        ensure!(
            catalog.roles.len() == PLAYER_COUNT,
            "PAL4 requires four roles"
        );

        let table = required_table(database, "UpgradeData")?;
        for values in &table.rows {
            let row = Row { table, values };
            let slot = role_slot(row.integer("ROLE_ID")?)?;
            let level = row.integer("CA_LEVEL")?;
            ensure!((1..=99).contains(&level), "invalid PAL4 level {level}");
            let entry = LevelDefinition {
                experience: row.integer("CA_EXPERIENCE")?,
                stats: row.stats()?,
            };
            ensure!(
                catalog.levels.insert((slot, level), entry).is_none(),
                "duplicate progression row for player {slot}, level {level}"
            );
        }
        for slot in 0..PLAYER_COUNT {
            ensure!(
                catalog.levels.contains_key(&(slot, 1)),
                "missing level one for player {slot}"
            );
        }

        let table = required_table(database, "Equip")?;
        for values in &table.rows {
            let row = Row { table, values };
            let id = row.integer("ID")?;
            let equipment = EquipmentDefinition {
                id,
                kind: EquipmentType::read(row.integer("TYPE")?)?,
                name: row.text("EA_NAME")?,
                description: row.text("EA_DESCRIPTION")?,
                model: row.text("EA_MODEL")?,
                texture: row.text("EA_TEXTURE")?,
                level_limits: row.player_integers("_LV_LMT")?,
                price: row.integer("EA_PRICE")?,
                skill_id: row.integer("EA_SKILL_ID")?,
                stats: row.stats()?,
            };
            ensure!(
                catalog.equipment.insert(id, equipment).is_none(),
                "duplicate equipment {id}"
            );
        }
        let table = required_table(database, "Property")?;
        for values in &table.rows {
            let row = Row { table, values };
            let id = row.integer("ID")?;
            ensure!(
                !catalog.equipment.contains_key(&id),
                "ambiguous item ID {id}"
            );
            let property = PropertyDefinition {
                id,
                name: row.text("PA_NAME")?,
                description: row.text("PA_DESCRIPTION")?,
                detail: row.text("PA_PROPERTY")?,
                category: row.integer("TYPE")?,
                target: row.integer("TARGET")?,
                model: row.text("PA_MODEL")?,
                texture: row.text("PA_TEXTURE")?,
                usable_by: row.player_integers("_CAN_USE")?.map(|value| value != 0),
                usable_in_menu: row.integer("CANUSE_IN_SYSUI")? != 0,
                droppable: row.integer("CANDROP")? != 0,
                price: row.integer("PA_PRICE")?,
                skill_id: row.integer("PA_ATTACHED_SKILL")?,
            };
            ensure!(
                catalog.properties.insert(id, property).is_none(),
                "duplicate property {id}"
            );
        }
        let table = required_table(database, "Skill")?;
        for values in &table.rows {
            let row = Row { table, values };
            let id = row.integer("ID")?;
            let mut parameters = [0.0; 9];
            for (index, parameter) in parameters.iter_mut().enumerate() {
                *parameter = match row.value(&format!("SKILL_PARAM_{}", index + 1))? {
                    Value::Float(value) if value.is_finite() => *value,
                    _ => bail!("invalid PAL4 skill parameter for {id}"),
                };
            }
            let skill = SkillDefinition {
                kind: row.integer("TYPE")?,
                trigger: row.integer("TRIGER")?,
                duration: row.integer("EXIST_TIME")?,
                function: row.integer("FUNCTION")?,
                parameters,
                attached: row.integer("ATTACHED_SKILL")?,
                mutex: row.integer("MUTEX_ID")?,
                status: row.integer("STATUS_ID")?,
            };
            ensure!(
                catalog.skills.insert(id, skill).is_none(),
                "duplicate skill {id}"
            );
        }
        if let Some(table) = database.tables.iter().find(|t| t.name == "Scene") {
            for values in &table.rows {
                let row = Row { table, values };
                catalog.scene_names.insert(
                    (row.text("SCENE")?, row.text("SECTION")?),
                    row.text("NAME")?,
                );
            }
        }
        if let Some(table) = database.tables.iter().find(|t| t.name == "StoryDesc") {
            for values in &table.rows {
                let row = Row { table, values };
                catalog.story_descriptions.insert(
                    row.integer("ID")?,
                    (row.text("CHAPTER")?, row.text("MEMO")?),
                );
            }
        }
        Ok(catalog)
    }
}

fn role_slot(id: i32) -> Result<usize> {
    ensure!((1001..=1004).contains(&id), "invalid PAL4 role ID {id}");
    Ok((id - 1001) as usize)
}

fn required_table<'a>(database: &'a Database, name: &str) -> Result<&'a Table> {
    database
        .tables
        .iter()
        .find(|table| table.name == name)
        .with_context(|| format!("missing PAL4 table {name}"))
}

struct Row<'a> {
    table: &'a Table,
    values: &'a [Value],
}

impl Row<'_> {
    fn value(&self, name: &str) -> Result<&Value> {
        let index = self
            .table
            .columns
            .iter()
            .position(|column| column.name == name)
            .with_context(|| format!("missing {}.{name}", self.table.name))?;
        self.values
            .get(index)
            .context("truncated PAL4 database row")
    }

    fn integer(&self, name: &str) -> Result<i32> {
        match self.value(name)? {
            Value::Integer(value) => Ok(*value),
            _ => bail!("{}.{} is not an integer", self.table.name, name),
        }
    }

    fn text(&self, name: &str) -> Result<String> {
        match self.value(name)? {
            Value::Text(value) => Ok(value.clone()),
            _ => bail!("{}.{} is not text", self.table.name, name),
        }
    }

    fn player_integers(&self, suffix: &str) -> Result<[i32; PLAYER_COUNT]> {
        let mut values = [0; PLAYER_COUNT];
        for (slot, name) in ["TIANHE", "LINSHA", "MENGLI", "ZIYING"].iter().enumerate() {
            values[slot] = self.integer(&format!("ePT_{name}{suffix}"))?;
        }
        Ok(values)
    }

    fn stats(&self) -> Result<CharacterStats> {
        let mut stats = CharacterStats::default();
        for (column, value) in self.table.columns.iter().zip(self.values) {
            let name = &column.name;
            if !(name.starts_with("CA_") || name.starts_with("EFE_"))
                || name == "CA_LEVEL"
                || name == "CA_EXPERIENCE"
            {
                continue;
            }
            match value {
                Value::Integer(value) => {
                    stats.integers.insert(name.clone(), *value);
                }
                Value::Float(value) => {
                    ensure!(
                        value.is_finite(),
                        "non-finite stat {}.{name}",
                        self.table.name
                    );
                    stats.rates.insert(name.clone(), *value);
                }
                _ => bail!("{}.{} is not a numeric stat", self.table.name, name),
            }
        }
        for name in [
            "CA_MAX_HP",
            "CA_MAX_MP",
            "CA_PHYSICAL",
            "CA_TOUGHNESS",
            "CA_SPEED",
            "CA_LUCKY",
            "CA_WILL",
        ] {
            ensure!(
                stats.integers.contains_key(name),
                "missing stat {}.{name}",
                self.table.name
            );
        }
        Ok(stats)
    }
}
