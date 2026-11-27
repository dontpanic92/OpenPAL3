use radiance::math::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::openpal3::loaders::stats::StatsData;
use crate::openpal3::scene::MechanismState;
use crate::ydirs;

use super::inventory::{InventoryState, linked_items};
use super::party::PartyState;
use super::stats::{Resource, StatsState};

pub const PAL3_APP_NAME: &str = "OpenPAL3";

#[derive(Serialize, Deserialize)]
pub struct PersistentState {
    app_name: String,
    global_vars: HashMap<i16, i32>,
    position: Vec3,
    #[serde(default)]
    nav_layer: usize,
    #[serde(default)]
    role_controlled: i32,
    party: PartyState,
    money: i32,
    #[serde(default)]
    favor: [i32; 5],
    play_seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thumbnail_png: Option<Vec<u8>>,
    stats: StatsState,
    inventory: InventoryState,
    scene: Option<String>,
    sub_scene: Option<String>,
    #[serde(default)]
    scene_mechanisms: HashMap<String, HashMap<u16, MechanismState>>,
    #[serde(default)]
    scene_object_visibility: HashMap<String, HashMap<u16, bool>>,
}

impl PersistentState {
    pub fn new(app_name: String) -> Self {
        Self {
            app_name,
            global_vars: HashMap::new(),
            position: Vec3::new(0., 0., 0.),
            nav_layer: 0,
            role_controlled: 0,
            party: PartyState::default(),
            money: 0,
            favor: [0; 5],
            play_seconds: 0.0,
            thumbnail_png: None,
            stats: StatsState::default(),
            inventory: InventoryState::default(),
            scene: None,
            sub_scene: None,
            scene_mechanisms: HashMap::new(),
            scene_object_visibility: HashMap::new(),
        }
    }

    fn get_data_dir(app_name: &str) -> PathBuf {
        ydirs::save_dir().join(app_name)
    }

    pub fn try_load(app_name: &str, slot: i32) -> anyhow::Result<Self> {
        let path = Self::get_data_dir(app_name)
            .join("Save")
            .join(format!("{}.json", slot));
        Self::load_from_path(app_name, &path)
    }

    pub(crate) fn load_from_path(app_name: &str, path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut state: Self = serde_json::from_str(&content)?;
        anyhow::ensure!(
            state.scene.as_ref().is_some_and(|s| !s.is_empty())
                && state.sub_scene.as_ref().is_some_and(|s| !s.is_empty()),
            "Save has no scene"
        );
        state.app_name = app_name.to_owned();
        Ok(state)
    }

    pub fn try_save(&self, slot: i32) -> anyhow::Result<()> {
        anyhow::ensure!(slot >= 0, "Invalid save slot");
        let dir = Self::get_data_dir(&self.app_name).join("Save");
        self.save_to_path(&dir.join(format!("{slot}.json")))
    }

    pub(crate) fn set_thumbnail(&mut self, png: Option<Vec<u8>>) {
        self.thumbnail_png = png;
    }

    pub(crate) fn thumbnail(&self) -> Option<&[u8]> {
        self.thumbnail_png.as_deref()
    }

    pub(crate) fn save_to_path(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&temporary, content)?;
        if let Err(error) = std::fs::rename(&temporary, path) {
            let _ = std::fs::remove_file(&temporary);
            return Err(error.into());
        }
        Ok(())
    }

    pub fn save(&self, slot: i32) {
        if slot < 0 {
            return;
        }
        if let Err(error) = self.try_save(slot) {
            log::error!("Cannot save slot {slot}: {error}");
        } else {
            log::info!("Game saved");
        }
    }

    pub fn app_name(&self) -> &str {
        self.app_name.as_str()
    }

    pub fn set_global(&mut self, var: i16, value: i32) {
        self.global_vars.insert(var, value);
    }

    pub fn get_global(&self, var: i16) -> Option<i32> {
        self.global_vars.get(&var).and_then(|v| Some(*v))
    }

    pub fn position(&mut self) -> Vec3 {
        self.position
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }

    pub fn nav_layer(&self) -> usize {
        self.nav_layer
    }

    pub fn set_nav_layer(&mut self, layer: usize) {
        self.nav_layer = layer;
    }

    pub fn role_controlled(&self) -> i32 {
        self.role_controlled
    }

    pub fn set_role_controlled(&mut self, role: i32) {
        self.role_controlled = role;
    }

    pub fn party(&self) -> &PartyState {
        &self.party
    }

    pub fn party_mut(&mut self) -> &mut PartyState {
        &mut self.party
    }

    pub fn money(&self) -> i32 {
        self.money
    }

    pub fn play_seconds(&self) -> i32 {
        self.play_seconds as i32
    }

    pub fn advance_play_time(&mut self, delta_sec: f32) {
        if delta_sec.is_finite() && delta_sec > 0.0 {
            self.play_seconds += f64::from(delta_sec);
        }
    }

    pub fn add_money(&mut self, amount: i32) {
        // Native 0x40fc90 adds signed 32-bit values before clamping below zero.
        self.money = self.money.wrapping_add(amount).max(0);
    }

    pub fn add_favor(&mut self, role: i32, amount: i32) {
        if let Some(value) = usize::try_from(role)
            .ok()
            .and_then(|role| self.favor.get_mut(role))
        {
            // Original 0x40eb10 performs an unclamped signed addition.
            *value = value.wrapping_add(amount);
        }
    }

    pub fn stats(&self) -> &StatsState {
        &self.stats
    }

    pub fn stats_mut(&mut self) -> &mut StatsState {
        &mut self.stats
    }

    pub fn inventory(&self) -> &InventoryState {
        &self.inventory
    }

    pub(crate) fn consume_world_item(&mut self, id: u32) -> bool {
        if !self.inventory.contains(id) {
            return false;
        }
        self.inventory.remove_one(id);
        true
    }

    pub fn can_equip_item(&self, role: i32, slot: i32, id: i32, data: &StatsData) -> bool {
        if !self.party.contains(role) || !(0..6).contains(&slot) {
            return false;
        }
        let current = self.stats.character(role).unwrap().equipment()[slot as usize];
        if id == 0 {
            return current.is_some();
        }
        let Some(item) = data.item(id as u32) else {
            return false;
        };
        item.category == [7, 8, 9, 10, 10, 11][slot as usize]
            && item.usable_by[role as usize]
            && self.inventory.contains(item.id)
            && current != Some(item.id)
    }

    pub fn equip_item(&mut self, role: i32, slot: i32, id: i32, data: &StatsData) -> bool {
        if !self.can_equip_item(role, slot, id, data) {
            return false;
        }
        let previous = self.stats.character(role).unwrap().equipment()[slot as usize];
        // Native 0x494880 swaps bag copies without SCE's automatic weapons or linked-item cleanup.
        if id != 0 {
            self.inventory.remove_one(id as u32);
        }
        self.stats.set_equipment(
            role,
            slot as usize,
            if id == 0 { None } else { Some(id as u32) },
            data,
        );
        if let Some(previous) = previous {
            self.inventory.add_stack(previous, 1);
        }
        true
    }

    pub fn item_use_scope(&self, id: i32, data: &StatsData) -> i32 {
        let Some(item) = data.item(id as u32) else {
            return 0;
        };
        if !matches!(item.category, 1 | 3)
            || item.use_context & 2 == 0
            || !matches!(item.use_scope, 1 | 2)
            || !matches!(item.use_kind, 0 | 12..=15)
            || item.conditions[27..].iter().any(|&effect| effect != 0)
        {
            return 0;
        }
        item.use_scope as i32
    }

    pub fn can_use_item(&self, role: i32, id: i32, data: &StatsData) -> bool {
        if !self.inventory.contains(id as u32) {
            return false;
        }
        let scope = self.item_use_scope(id, data);
        let Some(item) = data.item(id as u32) else {
            return false;
        };
        match scope {
            1 => self.party.contains(role) && self.stats.item_applies(role, item, data),
            2 if role == -1 => self
                .party
                .members()
                .any(|role| self.stats.item_applies(role, item, data)),
            _ => false,
        }
    }

    pub fn use_item(&mut self, role: i32, id: i32, data: &StatsData) -> bool {
        if !self.can_use_item(role, id, data) {
            return false;
        }
        let item = data.item(id as u32).unwrap();
        if item.use_scope == 2 {
            for role in self.party.members() {
                if self.stats.item_applies(role, item, data) {
                    self.stats.apply_item(role, item, data);
                }
            }
        } else {
            self.stats.apply_item(role, item, data);
        }
        self.inventory.remove_one(item.id);
        true
    }

    pub fn grant_world_item(&mut self, item_id: i32, data: &StatsData) -> bool {
        // Native pickup 0x42bef0 adds directly to the bag, without SCE grant side effects.
        let id = item_id as u32;
        if data.item(id).is_none() {
            return false;
        }
        self.inventory.add_stack(id, 1);
        true
    }

    pub fn add_item(&mut self, item_id: i32, count: i32, data: &StatsData) -> bool {
        let id = item_id as u32;
        let Some(item) = data.item(id) else {
            return false;
        };
        // Native AddItem auto-equips only weapons usable by Jing Tian.
        if item.category == 11 && item.usable_by[0] {
            let current = self.stats.character(0).unwrap().equipment()[5];
            let equipped = match current {
                Some(6603) => {
                    self.inventory.add_stack(id, count);
                    6603
                }
                // While Zhenyaojian is equipped, other grants are discarded except Mojian.
                Some(6602) if id != 6603 => 6602,
                _ => {
                    if let Some(previous) = current {
                        self.inventory.add_stack(previous, 1);
                    }
                    id
                }
            };
            self.stats.set_jingtian_weapon(equipped, data);
        } else {
            self.inventory.add_stack(id, count);
            for &linked in linked_items(id) {
                self.inventory.add_stack(linked, 1);
            }
        }
        true
    }

    pub fn remove_item(&mut self, item_id: i32, data: &StatsData) -> bool {
        let id = item_id as u32;
        if data.item(id).is_none() {
            return false;
        }
        self.stats.remove_equipped_item(id, 0..6);
        self.inventory.remove_one(id);
        for &linked in linked_items(id) {
            self.inventory.remove_one(linked);
        }
        // The extra earth-pearl consumable (6427) is removed only from the bag.
        for &linked in linked_items(id).iter().take(1) {
            self.stats.remove_equipped_item(linked, 3..5);
        }
        true
    }

    pub fn restore_attribute(&mut self, role_id: i32, selector: i32, data: &StatsData) -> bool {
        let Ok(resource) = Resource::try_from(selector) else {
            return false;
        };
        // FullRoleAtt truncates role IDs to u16; 6 selects current party members.
        let role_id = i32::from(role_id as u16);
        if role_id == 6 {
            for role in self.party.members() {
                self.stats.restore_resource(role, resource, data);
            }
            true
        } else {
            self.stats.restore_resource(role_id, resource, data)
        }
    }

    pub fn restore_team_attributes(&mut self, data: &StatsData) {
        for selector in 0..3 {
            self.restore_attribute(6, selector, data);
        }
    }

    pub fn average_level(&mut self, role_id: i32, offset: i32, data: &StatsData) -> bool {
        self.stats.average_level(&self.party, role_id, offset, data)
    }

    pub fn set_scene_name(&mut self, scene: String, sub_scene: String) {
        self.scene = Some(scene);
        self.sub_scene = Some(sub_scene);
    }

    pub fn scene_name(&self) -> Option<String> {
        self.scene.clone()
    }

    pub fn sub_scene_name(&self) -> Option<String> {
        self.sub_scene.clone()
    }

    pub fn mechanism_states(
        &self,
        scene: &str,
        sub_scene: &str,
    ) -> Option<&HashMap<u16, MechanismState>> {
        self.scene_mechanisms
            .get(&format!("{}/{}", scene, sub_scene).to_lowercase())
    }

    pub fn set_mechanism_states(
        &mut self,
        scene: &str,
        sub_scene: &str,
        states: HashMap<u16, MechanismState>,
    ) {
        self.scene_mechanisms
            .insert(format!("{}/{}", scene, sub_scene).to_lowercase(), states);
    }

    pub fn object_visibility(&self, scene: &str, sub_scene: &str) -> Option<&HashMap<u16, bool>> {
        self.scene_object_visibility
            .get(&format!("{}/{}", scene, sub_scene).to_lowercase())
    }

    pub fn set_object_visibility(
        &mut self,
        scene: &str,
        sub_scene: &str,
        visibility: HashMap<u16, bool>,
    ) {
        self.scene_object_visibility.insert(
            format!("{}/{}", scene, sub_scene).to_lowercase(),
            visibility,
        );
    }
}

#[cfg(test)]
mod gameplay_tests {
    use super::*;
    use crate::openpal3::{
        loaders::stats::test_data,
        states::stats::{Attributes, Resources},
    };
    use fileformats::pal3::gdb::ItemDefinition;

    fn inventory_data() -> StatsData {
        let medicine = ItemDefinition {
            id: 6201,
            category: 1,
            use_scope: 1,
            use_context: 3,
            amounts: [100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            ..Default::default()
        };
        let mut poison = ItemDefinition {
            id: 6226,
            amounts: [0; 12],
            ..medicine.clone()
        };
        poison.conditions[2] = 2;
        let mut world = ItemDefinition {
            id: 6402,
            category: 3,
            use_scope: 2,
            ..medicine.clone()
        };
        world.conditions[27] = 1;
        crate::openpal3::loaders::stats::test_data_with_items(&[
            medicine.clone(),
            ItemDefinition {
                id: 6211,
                modes: [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                amounts: [10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                ..medicine.clone()
            },
            ItemDefinition {
                id: 6414,
                category: 3,
                use_kind: 12,
                modes: [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                amounts: [10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                ..medicine.clone()
            },
            ItemDefinition {
                id: 6418,
                category: 3,
                modes: [0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0],
                amounts: [0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0],
                ..medicine.clone()
            },
            ItemDefinition {
                id: 6426,
                category: 3,
                use_kind: 15,
                amounts: [0; 12],
                ..medicine.clone()
            },
            ItemDefinition {
                id: 6231,
                use_context: 1,
                ..medicine.clone()
            },
            ItemDefinition {
                id: 6410,
                category: 3,
                use_kind: 8,
                ..medicine.clone()
            },
            ItemDefinition {
                id: 6214,
                amounts: [0; 12],
                ..medicine.clone()
            },
            ItemDefinition {
                id: 6209,
                use_scope: 2,
                ..medicine
            },
            ItemDefinition {
                id: 7000,
                category: 7,
                usable_by: [false, true, false, false, false],
                ..Default::default()
            },
            ItemDefinition {
                id: 8146,
                category: 10,
                usable_by: [true; 5],
                amounts: [15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                ..Default::default()
            },
            poison,
            world,
        ])
    }

    #[test]
    fn manual_equipment_transfers_preserve_other_owners_and_clamp_without_sce_side_effects() {
        let data = inventory_data();
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        state.party.set_in_team(1, true);
        state.inventory.add_stack(8146, 3);
        for (role, slot) in [(0, 3), (0, 4), (1, 3)] {
            assert!(state.equip_item(role, slot, 8146, &data));
        }
        assert!(!state.inventory.contains(8146));
        assert!(!state.equip_item(1, 4, 8146, &data));
        state.inventory.add_stack(8146, 1);
        state.inventory.add_stack(7000, 1);
        let before = serde_json::to_string(&state).unwrap();
        for (role, slot, id) in [
            (0, 3, 8146),
            (-1, 3, 8146),
            (2, 3, 8146),
            (0, 6, 8146),
            (0, 0, 8146),
            (0, 3, -1),
            (0, 0, 7000),
        ] {
            assert!(!state.equip_item(role, slot, id, &data));
        }
        assert_eq!(serde_json::to_string(&state).unwrap(), before);
        assert!(state.equip_item(1, 0, 7000, &data));
        state.stats.restore_resource(0, Resource::Hp, &data);
        let max = state.stats.attributes(0, &data).unwrap().hp;
        assert!(state.equip_item(0, 3, 0, &data));
        assert_eq!(state.inventory.quantity(8146), 2);
        assert_eq!(state.stats.character(0).unwrap().current().hp, max - 15);
        assert_eq!(state.stats.character(0).unwrap().equipment()[4], Some(8146));
        assert_eq!(state.stats.character(1).unwrap().equipment()[3], Some(8146));
        state.inventory.add_stack(6603, 1);
        state.inventory.add_stack(6601, -2);
        assert!(state.equip_item(0, 5, 6603, &data));
        assert!(state.equip_item(0, 5, 6601, &data));
        assert_eq!(state.stats.character(0).unwrap().equipment()[5], Some(6601));
        assert_eq!(state.inventory.quantity(6603), 1);
        assert!(!state.inventory.contains(6601));
        assert!(state.equip_item(0, 5, 0, &data));
        assert_eq!(state.stats.character(0).unwrap().equipment()[5], None);
        assert_eq!(state.inventory.quantity(6601), 1);
    }

    #[test]
    fn adventure_items_match_native_healing_rounding_poison_and_permanent_growth() {
        let data = inventory_data();
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        let base_hp = state.stats.character(0).unwrap().base().hp;
        state.stats.add_base_attributes(
            0,
            Attributes {
                hp: 85 - base_hp,
                ..Default::default()
            },
            &data,
        );
        state.stats.set_resource(0, Resource::Hp, 0, &data);
        state.inventory.add_stack(6211, 2);
        assert!(state.use_item(0, 6211, &data));
        assert_eq!(state.stats.character(0).unwrap().current().hp, 8);
        assert!(state.use_item(0, 6211, &data));
        assert_eq!(state.stats.character(0).unwrap().current().hp, 16);
        state.inventory.add_stack(6414, 1);
        assert!(state.use_item(0, 6414, &data));
        assert_eq!(state.stats.character(0).unwrap().base().hp, 95);
        assert_eq!(state.stats.character(0).unwrap().current().hp, 16);
        let attack = state.stats.attributes(0, &data).unwrap().attack;
        state.inventory.add_stack(6418, 1);
        assert!(state.use_item(0, 6418, &data));
        assert_eq!(state.stats.attributes(0, &data).unwrap().attack, attack + 2);
        assert_eq!(state.stats.character(0).unwrap().current().hp, 16);
        state.stats.set_poison(0, 2, 1);
        state.stats.set_poison(0, 3, 1);
        state.inventory.add_stack(6226, 2);
        assert!(state.use_item(0, 6226, &data));
        assert_eq!(
            state.stats.character(0).unwrap().poisons(),
            &[0, 0, 0, 1, 0]
        );
        assert!(!state.use_item(0, 6226, &data));
        assert_eq!(state.inventory.quantity(6226), 1);
        let experience = state.stats.character(0).unwrap().experience();
        state.inventory.add_stack(6426, 2);
        assert!(state.use_item(0, 6426, &data));
        assert_eq!(state.stats.character(0).unwrap().level(), 2);
        assert_eq!(state.stats.character(0).unwrap().experience(), experience);
        assert_eq!(state.stats.character(0).unwrap().current().hp, 16);
        let restored: PersistentState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(restored.stats, state.stats);
        assert_eq!(restored.inventory.stacks(), state.inventory.stacks());
    }

    #[test]
    fn unsupported_or_inapplicable_items_never_consume_and_party_use_consumes_once() {
        let data = inventory_data();
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        for id in [6201, 6214, 6231, 6402, 6410, 6209] {
            state.inventory.add_stack(id, 3);
        }
        let before = serde_json::to_string(&state).unwrap();
        for (role, id) in [
            (0, 6201),
            (0, 6214),
            (0, 6231),
            (-1, 6402),
            (0, 6410),
            (-1, 6201),
            (1, 6201),
            (0, 999),
            (0, 6209),
        ] {
            assert!(!state.use_item(role, id, &data));
        }
        assert_eq!(serde_json::to_string(&state).unwrap(), before);
        state.party.set_in_team(1, true);
        state.stats.set_resource(0, Resource::Hp, 0, &data);
        state.stats.set_resource(1, Resource::Hp, 0, &data);
        state.stats.set_resource(2, Resource::Hp, 0, &data);
        assert!(state.use_item(-1, 6209, &data));
        for role in [0, 1] {
            assert_eq!(
                state.stats.character(role).unwrap().current().hp,
                state.stats.attributes(role, &data).unwrap().hp.min(100)
            );
        }
        assert_eq!(state.stats.character(2).unwrap().current().hp, 0);
        assert_eq!(state.inventory.quantity(6209), 2);
        while state.stats.character(0).unwrap().level() < 99 {
            state.stats.advance_level(0, &data);
        }
        state.inventory.add_stack(6426, 1);
        assert!(state.use_item(0, 6426, &data));
        assert_eq!(state.stats.character(0).unwrap().level(), 99);
        assert!(!state.inventory.contains(6426));
        state.average_level(0, 99, &data);
        assert_eq!(state.stats.character(0).unwrap().level(), 100);
        let experience = state.stats.character(0).unwrap().experience();
        state.inventory.add_stack(6426, 1);
        assert!(state.use_item(0, 6426, &data));
        assert_eq!(state.stats.character(0).unwrap().level(), 99);
        assert_eq!(state.stats.character(0).unwrap().experience(), experience);
    }

    #[test]
    fn play_time_preserves_fractional_seconds_across_save_and_ignores_invalid_deltas() {
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        state.advance_play_time(0.75);
        assert_eq!(state.play_seconds(), 0);
        let mut restored: PersistentState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        for delta in [-1.0, f32::NAN, f32::INFINITY] {
            restored.advance_play_time(delta);
        }
        restored.advance_play_time(0.5);
        assert_eq!(restored.play_seconds(), 1);
    }

    #[test]
    fn inventory_commands_remove_one_copy_and_clear_even_inactive_roles_equipment() {
        let data = test_data();
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        assert!(state.inventory().stacks().is_empty());
        state.add_item(42, 3, &data);
        state.add_item(6008, 2, &data);
        state.remove_item(42, &data);
        assert_eq!(state.inventory().quantity(42), 2);
        assert_eq!(state.inventory().quantity(6008), 2);
        assert!(!state.party().contains(1));
        assert_eq!(state.stats().attributes(1, &data).unwrap().attack, 35);
        state.remove_item(82201, &data);
        assert_eq!(state.stats().character(1).unwrap().equipment()[5], None);
        assert_eq!(state.stats().attributes(1, &data).unwrap().attack, 28);
        let before = serde_json::to_string(&state).unwrap();
        assert!(!state.add_item(-1, 1, &data));
        assert!(!state.remove_item(123456, &data));
        assert_eq!(serde_json::to_string(&state).unwrap(), before);
    }

    #[test]
    fn pearl_grants_and_removals_apply_linked_items_once_per_command() {
        let data = test_data();
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        for (primary, linked) in [
            (6119, 8146),
            (6120, 8147),
            (6122, 8149),
            (6123, 8150),
            (6124, 8151),
            (6125, 8152),
        ] {
            state.add_item(primary, 3, &data);
            assert_eq!(state.inventory().quantity(primary as u32), 3);
            assert_eq!(state.inventory().quantity(linked), 1);
            state.remove_item(primary, &data);
            assert_eq!(state.inventory().quantity(primary as u32), 2);
            assert!(!state.inventory().contains(linked));
        }
        assert!(!state.inventory().contains(6427));
        state.add_item(8146, 2, &data);
        for _ in 0..3 {
            state.remove_item(6119, &data);
        }
        assert!(!state.inventory().contains(6119));
        assert!(!state.inventory().contains(8146));
    }

    #[test]
    fn world_pickups_add_one_without_script_equipment_or_linked_item_effects() {
        let data = test_data();
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        state.stats_mut().set_resource(0, Resource::Hp, 12, &data);
        let stats = state.stats().clone();
        for id in [6601, 6602, 6603, 6119, 6123] {
            assert!(state.grant_world_item(id, &data));
            assert_eq!(state.inventory().quantity(id as u32), 1);
        }
        assert_eq!(state.stats(), &stats);
        assert!(!state.inventory().contains(8146));
        assert!(!state.inventory().contains(8150));
        let before = serde_json::to_string(&state).unwrap();
        assert!(!state.grant_world_item(-1, &data));
        assert!(!state.grant_world_item(123456, &data));
        assert_eq!(serde_json::to_string(&state).unwrap(), before);
    }

    #[test]
    fn world_pickups_preserve_native_signed_stacks_and_upper_cap() {
        let data = test_data();
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        state.inventory.add_stack(42, -2);
        assert!(state.grant_world_item(42, &data));
        assert_eq!(state.inventory().quantity(42), -1);
        state.inventory.add_stack(42, 100);
        assert!(state.grant_world_item(42, &data));
        assert_eq!(state.inventory().quantity(42), 99);
        let restored: PersistentState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(restored.inventory(), state.inventory());
    }

    #[test]
    fn script_weapon_grants_obey_both_story_swords_and_preserve_injuries() {
        let data = test_data();
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        state.stats_mut().set_resource(0, Resource::Hp, 12, &data);
        for (id, equipped, wooden_in_bag) in [
            (6601, 6601, 0),
            (6602, 6602, 1),
            (6601, 6602, 1),
            (6603, 6603, 1),
            (6601, 6603, 2),
        ] {
            state.add_item(id, 1, &data);
            assert_eq!(
                state.stats().character(0).unwrap().equipment()[5],
                Some(equipped)
            );
            assert_eq!(state.inventory().quantity(6601), wooden_in_bag);
        }
        assert_eq!(state.inventory().quantity(6602), 1);
        assert!(!state.inventory().contains(6603));
        assert_eq!(state.stats().character(0).unwrap().current().hp, 12);
        assert_eq!(state.stats().attributes(0, &data).unwrap().attack, 65);
        assert_eq!(
            state.stats().attributes(0, &data).unwrap().elements,
            [3, 3, 7, 1, 2]
        );
        // A prior equipment removal may leave a pool above its newly reduced maximum.
        let mut saved = serde_json::to_value(&state).unwrap();
        saved["stats"]["characters"][0]["current"]["hp"] = 120.into();
        let mut state: PersistentState = serde_json::from_value(saved).unwrap();
        state.add_item(6601, 1, &data);
        assert_eq!(state.stats().character(0).unwrap().current().hp, 88);
        assert_eq!(
            state.stats().character(0).unwrap().equipment()[5],
            Some(6603)
        );
    }

    #[test]
    fn inventory_and_equipped_story_sword_survive_save_roundtrip() {
        let data = test_data();
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        state.add_item(6603, 1, &data);
        state.add_item(6123, 3, &data);
        state.add_item(42, 0, &data);
        state.remove_item(6123, &data);
        let mut restored: PersistentState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(restored.inventory(), state.inventory());
        assert_eq!(restored.stats(), state.stats());
        assert!(restored.inventory().contains(42));
        assert_eq!(restored.inventory().quantity(6123), 2);
        assert!(!restored.inventory().contains(6427));
        assert_eq!(restored.stats().attributes(0, &data).unwrap().attack, 65);
        restored.remove_item(6603, &data);
        assert_eq!(restored.stats().attributes(0, &data).unwrap().attack, 30);
    }

    #[test]
    fn restoration_uses_actual_party_and_resource_selector_without_recruiting_roles() {
        let data = test_data();
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        state.party_mut().set_in_team(2, true);
        for role in 0..5 {
            for resource in [Resource::Hp, Resource::Qi, Resource::Mp] {
                state.stats_mut().set_resource(role, resource, 1, &data);
            }
        }
        assert!(state.restore_attribute(6, 0, &data));
        assert_eq!(
            state.stats().character(0).unwrap().current(),
            Resources {
                hp: 88,
                qi: 1,
                mp: 1
            }
        );
        assert_eq!(state.stats().character(2).unwrap().current().hp, 206);
        assert_eq!(state.stats().character(1).unwrap().current().hp, 1);
        assert!(state.restore_attribute(1, 2, &data));
        assert_eq!(state.stats().character(1).unwrap().current().mp, 30);
        assert!(!state.party().contains(1));
        assert!(!state.restore_attribute(5, 0, &data));
        assert!(!state.restore_attribute(0, 3, &data));
        state.restore_team_attributes(&data);
        assert_eq!(
            state.stats().character(2).unwrap().current(),
            Resources {
                hp: 206,
                qi: 41,
                mp: 78
            }
        );
        assert_eq!(state.stats().character(1).unwrap().current().hp, 1);
    }

    #[test]
    fn stats_roundtrip_preserves_level_exp_bonuses_and_partial_resource_pools() {
        let data = test_data();
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        state.party_mut().set_in_team(3, true);
        state.average_level(1, 2, &data);
        state.stats_mut().add_base_attributes(
            1,
            Attributes {
                hp: 10,
                speed: 2,
                ..Default::default()
            },
            &data,
        );
        state.stats_mut().add_experience(1, 50);
        state.stats_mut().set_resource(1, Resource::Hp, 12, &data);
        state.stats_mut().set_resource(1, Resource::Mp, 3, &data);
        let json = serde_json::to_string(&state).unwrap();
        let restored: PersistentState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.stats(), state.stats());
        let character = restored.stats().character(1).unwrap();
        assert_eq!(character.level(), 7);
        assert_eq!(character.experience(), 650);
        assert_eq!(restored.stats().attributes(1, &data).unwrap().hp, 139);
        assert_eq!(character.current().hp, 12);
        assert_eq!(character.current().mp, 3);
        assert_eq!(character.equipment()[5], Some(82201));
    }

    #[test]
    fn party_and_money_roundtrip_independently_of_the_controlled_role() {
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        state.party_mut().set_in_team(1, true);
        state.party_mut().set_in_team(3, true);
        state.party_mut().set_in_team(1, false);
        state.set_role_controlled(1);
        state.add_money(250);
        state.add_money(-40);

        let json = serde_json::to_string(&state).unwrap();
        let mut restored: PersistentState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.role_controlled(), 1);
        assert_eq!(restored.party().members().collect::<Vec<_>>(), vec![0, 3]);
        assert_eq!(restored.party().formation_slot(1), Some(1));
        assert_eq!(restored.party().formation_slot(3), Some(2));
        assert_eq!(restored.money(), 210);

        restored.party_mut().set_in_team(4, true);
        assert_eq!(restored.party().formation_slot(4), Some(1));
        assert_eq!(restored.role_controlled(), 1);
    }

    #[test]
    fn money_starts_empty_and_clamps_after_signed_addition() {
        let mut state = PersistentState::new(PAL3_APP_NAME.into());
        assert_eq!(state.money(), 0);
        state.add_money(100);
        state.add_money(-150);
        assert_eq!(state.money(), 0);
        state.add_money(i32::MAX);
        assert_eq!(state.money(), i32::MAX);
        state.add_money(1);
        assert_eq!(state.money(), 0);
        state.add_money(i32::MIN);
        assert_eq!(state.money(), 0);
    }
}

#[cfg(test)]
mod mechanism_tests {
    use super::*;

    #[test]
    fn old_saves_without_mechanisms_remain_readable() {
        let mut json = serde_json::to_value(PersistentState::new("OpenPAL3".into())).unwrap();
        json.as_object_mut().unwrap().remove("scene_mechanisms");
        json.as_object_mut().unwrap().remove("nav_layer");
        json.as_object_mut().unwrap().remove("role_controlled");
        let state: PersistentState = serde_json::from_value(json).unwrap();
        assert!(state.mechanism_states("m17", "7").is_none());
    }

    #[test]
    fn mechanism_state_roundtrips_with_case_insensitive_scene_keys() {
        let mut state = PersistentState::new("OpenPAL3".into());
        state.set_role_controlled(1);
        state.set_nav_layer(1);
        state.set_mechanism_states(
            "M17",
            "D1",
            HashMap::from([(
                8,
                MechanismState {
                    remaining: 254,
                    switched: 1,
                    height: 25.,
                    raised: false,
                    motion: Some(40.),
                    animation_time: None,
                    native: None,
                    layer: None,
                    platform: None,
                    interaction: None,
                    action: None,
                    hazard: None,
                    chest: None,
                },
            )]),
        );
        let mut json = serde_json::to_value(&state).unwrap();
        for field in ["native", "layer", "platform", "interaction"] {
            json["scene_mechanisms"]["m17/d1"]["8"]
                .as_object_mut()
                .unwrap()
                .remove(field);
        }

        let restored: PersistentState = serde_json::from_value(json).unwrap();
        assert_eq!(restored.role_controlled(), 1);
        assert_eq!(restored.nav_layer(), 1);
        let mechanism = &restored.mechanism_states("m17", "d1").unwrap()[&8];
        assert_eq!(mechanism.switched, 1);
        assert_eq!(mechanism.remaining, 254);
        assert_eq!(mechanism.height, 25.);
        assert_eq!(mechanism.motion, Some(40.));
        assert!(restored.mechanism_states("m18", "d1").is_none());
    }

    #[test]
    fn favor_penalties_wrap_and_roundtrip_without_breaking_old_saves() {
        let mut state = PersistentState::new("OpenPAL3".into());
        state.add_favor(1, -2);
        state.add_favor(2, i32::MAX);
        state.add_favor(2, 1);
        state.add_favor(-1, 20);
        state.add_favor(5, 20);
        let json = serde_json::to_string(&state).unwrap();
        let restored: PersistentState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.favor, [0, -2, i32::MIN, 0, 0]);
        let mut old = serde_json::to_value(state).unwrap();
        old.as_object_mut().unwrap().remove("favor");
        let restored: PersistentState = serde_json::from_value(old).unwrap();
        assert_eq!(restored.favor, [0; 5]);
    }
}
