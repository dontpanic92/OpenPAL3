use std::collections::HashMap;
use std::path::PathBuf;

use radiance::math::Transform;
use radiance::math::Vec3;
use serde::{Deserialize, Serialize};

use crate::openpal4::gameplay::{CharacterStats, GameplayCatalog, PlayerEquipment};
use crate::ydirs;

/// Number of fixed party slots in PAL4 (YunTianhe / HanLingsha /
/// LiuMengli / MurongZiying). Mirrors the `Player` enum in
/// `openpal4::scene`.
pub const PLAYER_COUNT: usize = 4;

/// PAL4 save namespace. Save slots live under
/// `<save_dir>/<PAL4_APP_NAME>/Save/<slot>.json`. Single source of
/// truth shared by `Pal4VmContext` (which seeds a fresh
/// `Pal4PersistentState` with it) and `Pal4Service` (whose start-menu
/// load screen peeks the same slot files).
pub const PAL4_APP_NAME: &str = "OpenPAL4";

/// Per-player progression, separate from scene visibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    #[serde(default)]
    pub level: i32,
    #[serde(default)]
    pub hp: i32,
    #[serde(default)]
    pub max_hp: i32,
    #[serde(default)]
    pub mp: i32,
    #[serde(default)]
    pub max_mp: i32,
    #[serde(default)]
    pub in_team: bool,
    #[serde(default)]
    pub skills: Vec<i32>,
    #[serde(default)]
    pub equipment: PlayerEquipment,
    #[serde(default)]
    pub experience: i32,
    #[serde(default)]
    pub base_stats: CharacterStats,
    #[serde(default)]
    pub effective_stats: CharacterStats,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            level: 1,
            hp: 0,
            max_hp: 0,
            mp: 0,
            max_mp: 0,
            in_team: false,
            skills: Vec::new(),
            equipment: PlayerEquipment::default(),
            experience: 0,
            base_stats: CharacterStats::default(),
            effective_stats: CharacterStats::default(),
        }
    }
}

/// Serializable snapshot of PAL4 game progress. Saved as slot-based
/// JSON under `<save_dir>/<app_name>/Save/<slot>.json`, mirroring the
/// OpenPAL3 `PersistentState` convention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pal4PersistentState {
    app_name: String,
    #[serde(default)]
    money: i32,
    #[serde(default)]
    quest_percentage: i32,
    #[serde(default)]
    leader: usize,
    #[serde(default)]
    scene_name: String,
    #[serde(default)]
    block_name: String,
    /// PAL4 `giArenaLoad`'s third argument: the lettered sibling folder
    /// (e.g. `M06/1A`) that carries the block's gameplay data when the
    /// geometry block ships none. Empty ≡ "same folder as `block_name`".
    /// Absent in older saves.
    #[serde(default)]
    sub_block_name: String,
    #[serde(default)]
    position: Option<Vec3>,
    /// Leader facing direction (degrees, yaw about world-up) at save
    /// time, restored alongside `position` so the player stands facing
    /// the same way. `None` in older saves.
    #[serde(default)]
    direction: Option<f32>,
    /// Whether player control is locked (cutscene). A brand-new
    /// playthrough starts **locked** (`Pal4PersistentState::new` sets
    /// `true`) so the new-game intro plays out before the player gets
    /// control — this is the single source of truth, honoured both at
    /// `Pal4VmContext` construction and by the `giNewGame` reset.
    /// The `#[serde(default)]` is independent and yields `false` for
    /// older saves that lack the field (they were always taken during
    /// free movement, so they remain controllable after load).
    #[serde(default)]
    player_locked: bool,
    /// Full camera transform (position + orientation) at save time, so
    /// a load restores the exact view. `None` in older saves (and when
    /// no scene camera exists), in which case the loaded scene keeps
    /// its default camera. The PAL4 gameplay camera is static between
    /// cinematic camera runs, so persisting the transform is exact.
    #[serde(default)]
    camera: Option<Transform>,
    #[serde(default)]
    players: HashMap<usize, PlayerState>,
    /// Inventory / owned equipment as item-id -> count.
    #[serde(default)]
    inventory: HashMap<i32, i32>,
    /// Snapshot of the shared angelscript `ScriptGlobalContext.vars`,
    /// which hold cross-scene story-plot flags.
    #[serde(default)]
    script_globals: Vec<u32>,
}

impl Pal4PersistentState {
    /// Number of save slots surfaced by the start-menu load screen.
    /// Mirrors the in-game Num1-Num4 / Num5-Num8 save/load hotkeys.
    pub const SLOT_COUNT: i32 = 4;

    pub fn new(app_name: String) -> Self {
        let mut players = HashMap::new();
        for slot in 0..PLAYER_COUNT {
            players.insert(slot, PlayerState::default());
        }

        Self {
            app_name,
            money: 0,
            quest_percentage: 0,
            leader: 0,
            scene_name: String::new(),
            block_name: String::new(),
            sub_block_name: String::new(),
            position: None,
            camera: None,
            direction: None,
            // A fresh playthrough starts control-locked so the new-game
            // opening cutscene runs before the player gets control. The
            // serde default for this field stays `false` (older saves
            // without it were taken during free movement).
            player_locked: true,
            players,
            inventory: HashMap::new(),
            script_globals: Vec::new(),
        }
    }

    fn get_data_dir(app_name: &str) -> PathBuf {
        ydirs::save_dir().join(app_name)
    }

    /// Load the persistent state for `app_name` from the given slot.
    /// Returns an error if the slot file is missing or malformed; the
    /// caller decides whether to fall back to a fresh state.
    pub fn load(app_name: &str, slot: i32) -> anyhow::Result<Self> {
        let path = Self::get_data_dir(app_name)
            .join("Save")
            .join(format!("{}.json", slot));
        let content = std::fs::read_to_string(path)?;
        let state = serde_json::from_str(&content)?;
        Ok(state)
    }

    /// Read a save slot for display purposes only. Returns `None` when
    /// the slot file is missing or malformed (the start-menu load
    /// screen renders such slots as empty / non-selectable).
    pub fn peek(app_name: &str, slot: i32) -> Option<Self> {
        Self::load(app_name, slot).ok()
    }

    /// Short human-readable one-line summary of this save for the load
    /// screen: scene/block location plus quest completion. Falls back
    /// to a generic label when no scene was recorded.
    pub fn summary(&self) -> String {
        if self.scene_name.is_empty() {
            return format!("Quest {}%", self.quest_percentage);
        }
        if self.block_name.is_empty() {
            format!("{} - Quest {}%", self.scene_name, self.quest_percentage)
        } else {
            format!(
                "{}/{} - Quest {}%",
                self.scene_name, self.block_name, self.quest_percentage
            )
        }
    }

    /// Persist this state to the given slot. Negative slots are
    /// ignored (matching the OpenPAL3 "no slot selected" sentinel).
    pub fn save(&self, slot: i32) {
        if slot < 0 {
            return;
        }

        let path = Self::get_data_dir(&self.app_name).join("Save");
        if let Err(e) = std::fs::create_dir_all(&path) {
            log::error!("Cannot create save dir: {}", e);
            return;
        }

        match serde_json::to_string_pretty(self) {
            Ok(content) => {
                if let Err(e) = std::fs::write(path.join(format!("{}.json", slot)), content) {
                    log::error!("Cannot save: {}", e);
                } else {
                    log::info!("Game saved to slot {}", slot);
                }
            }
            Err(e) => log::error!("Cannot serialize persistent state: {}", e),
        }
    }

    pub fn app_name(&self) -> &str {
        self.app_name.as_str()
    }

    // --- Money ---------------------------------------------------------

    pub fn money(&self) -> i32 {
        self.money
    }

    pub fn add_money(&mut self, amount: i32) {
        self.money = self.money.saturating_add(amount);
    }

    /// Deduct `amount` from money, clamping at zero. Returns `true`
    /// when the player had enough to cover the full cost.
    pub fn pay_money(&mut self, amount: i32) -> bool {
        if self.money >= amount {
            self.money -= amount;
            true
        } else {
            self.money = 0;
            false
        }
    }

    // --- Quest ---------------------------------------------------------

    pub fn quest_percentage(&self) -> i32 {
        self.quest_percentage
    }

    pub fn add_quest_percentage(&mut self, delta: i32) {
        self.quest_percentage = (self.quest_percentage + delta).clamp(0, 100);
    }

    // --- Players -------------------------------------------------------

    pub fn player(&self, slot: usize) -> Option<&PlayerState> {
        self.players.get(&slot)
    }

    pub fn player_mut(&mut self, slot: usize) -> &mut PlayerState {
        self.players.entry(slot).or_default()
    }

    pub fn initialize_players(&mut self, catalog: &GameplayCatalog) -> anyhow::Result<()> {
        for slot in 0..PLAYER_COUNT {
            self.player_mut(slot).base_stats = catalog.roles[&slot].stats.clone();
            self.set_player_level(slot, 1, catalog)?;
        }
        self.set_in_team(0, true);
        Ok(())
    }

    pub fn set_player_level(
        &mut self,
        slot: usize,
        level: i32,
        catalog: &GameplayCatalog,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(slot < PLAYER_COUNT, "invalid PAL4 player {slot}");
        let definition = catalog
            .levels
            .get(&(slot, level))
            .ok_or_else(|| anyhow::anyhow!("missing PAL4 player {slot}, level {level}"))?;
        let lower = catalog
            .levels
            .get(&(slot, level - 1))
            .map(|row| row.experience)
            .unwrap_or(0);
        let upper = definition.experience.saturating_sub(1);
        anyhow::ensure!(
            lower <= upper,
            "invalid PAL4 experience bounds for level {level}"
        );
        let player = self.player_mut(slot);
        let previous = player.base_stats.clone();
        player.base_stats = definition.stats.clone();
        // Native level assignment preserves learned elemental affinities.
        for name in [
            "EFE_WATER",
            "EFE_FIRE",
            "EFE_THUNDER",
            "EFE_AIR",
            "EFE_EARTH",
        ] {
            player
                .base_stats
                .integers
                .insert(name.to_string(), previous.integer(name));
        }
        player.level = level;
        player.experience = player.experience.clamp(lower, upper);
        self.recalculate_player(slot, catalog)?;
        self.set_full_hp(slot);
        self.set_full_mp(slot);
        Ok(())
    }

    pub fn player_level(&self, slot: usize) -> i32 {
        self.players.get(&slot).map(|p| p.level).unwrap_or(1)
    }

    pub fn set_in_team(&mut self, slot: usize, in_team: bool) {
        if slot < PLAYER_COUNT {
            self.player_mut(slot).in_team = in_team;
        }
    }

    pub fn add_skill(&mut self, slot: usize, skill_id: i32) {
        let player = self.player_mut(slot);
        if !player.skills.contains(&skill_id) {
            player.skills.push(skill_id);
        }
    }

    /// Script grants create an equipped copy, without consuming backpack stock.
    pub fn add_player_equip(
        &mut self,
        slot: usize,
        equip_id: i32,
        catalog: &GameplayCatalog,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(slot < PLAYER_COUNT, "invalid PAL4 player {slot}");
        let definition = catalog
            .equipment
            .get(&equip_id)
            .ok_or_else(|| anyhow::anyhow!("unknown PAL4 equipment {equip_id}"))?;
        let position = self.player_mut(slot).equipment.script_slot(definition.kind);
        let previous = *self.player_mut(slot).equipment.at_mut(position);
        if let Some(previous) = previous {
            anyhow::ensure!(
                self.equipment_count(previous) < i32::MAX,
                "PAL4 backpack quantity overflow for {previous}"
            );
        }
        *self.player_mut(slot).equipment.at_mut(position) = Some(equip_id);
        if let Some(previous) = previous {
            self.add_equipment(previous, 1);
        }
        self.recalculate_player(slot, catalog)
    }

    pub fn recalculate_player(
        &mut self,
        slot: usize,
        catalog: &GameplayCatalog,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(slot < PLAYER_COUNT, "invalid PAL4 player {slot}");
        let player = self.player_mut(slot);
        let mut effective = player.base_stats.clone();
        for id in player.equipment.ids() {
            let equipment = catalog
                .equipment
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("unknown equipped PAL4 item {id}"))?;
            effective.add(&equipment.stats);
        }
        player.max_hp = effective.integer("CA_MAX_HP").max(0);
        player.max_mp = effective.integer("CA_MAX_MP").max(0);
        player.hp = player.hp.clamp(0, player.max_hp);
        player.mp = player.mp.clamp(0, player.max_mp);
        player.effective_stats = effective;
        Ok(())
    }

    pub fn equip_from_inventory(
        &mut self,
        slot: usize,
        id: i32,
        catalog: &GameplayCatalog,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(slot < PLAYER_COUNT, "invalid PAL4 player {slot}");
        let player = self
            .player(slot)
            .ok_or_else(|| anyhow::anyhow!("missing PAL4 player {slot}"))?;
        anyhow::ensure!(
            catalog.can_equip(slot, player.level, id),
            "player {slot} cannot equip {id}"
        );
        anyhow::ensure!(
            self.equipment_count(id) > 0,
            "PAL4 equipment {id} is not in the backpack"
        );

        // Validate on a copy so a failed replacement cannot consume either item.
        let mut candidate = self.clone();
        let position = candidate
            .player_mut(slot)
            .equipment
            .script_slot(catalog.equipment[&id].kind);
        let previous = candidate
            .player_mut(slot)
            .equipment
            .at_mut(position)
            .replace(id);
        candidate.remove_equipment(id, 1);
        if let Some(previous) = previous {
            anyhow::ensure!(
                candidate.equipment_count(previous) < i32::MAX,
                "PAL4 backpack quantity overflow for {previous}"
            );
            candidate.add_equipment(previous, 1);
        }
        candidate.recalculate_player(slot, catalog)?;
        *self = candidate;
        Ok(())
    }

    pub fn set_player_resource_percent(&mut self, player_id: i32, attribute: i32, percent: i32) {
        if !(-1..=1).contains(&attribute) {
            return;
        }
        for slot in 0..PLAYER_COUNT {
            if player_id >= 0 && player_id as usize != slot {
                continue;
            }
            let player = self.player_mut(slot);
            let ratio = percent as f32 * 0.01;
            if attribute == -1 || attribute == 0 {
                player.hp =
                    ((f64::from(player.max_hp) * f64::from(ratio)) as i32).clamp(0, player.max_hp);
            }
            if attribute == -1 || attribute == 1 {
                player.mp =
                    ((f64::from(player.max_mp) * f64::from(ratio)) as i32).clamp(0, player.max_mp);
            }
        }
    }

    pub fn restore_party_resources(&mut self, hp: bool) {
        for player in self.players.values_mut().filter(|player| player.in_team) {
            if hp {
                player.hp = player.max_hp;
            } else {
                player.mp = player.max_mp;
            }
        }
    }

    pub fn set_full_hp(&mut self, slot: usize) {
        let player = self.player_mut(slot);
        player.hp = player.max_hp;
    }

    pub fn set_full_mp(&mut self, slot: usize) {
        let player = self.player_mut(slot);
        player.mp = player.max_mp;
    }

    // --- Inventory / equipment ----------------------------------------

    pub fn add_equipment(&mut self, equip_id: i32, count: i32) {
        let entry = self.inventory.entry(equip_id).or_insert(0);
        *entry = entry.saturating_add(count).max(0);
        if *entry == 0 {
            self.inventory.remove(&equip_id);
        }
    }

    pub fn remove_equipment(&mut self, equip_id: i32, count: i32) {
        self.add_equipment(equip_id, count.saturating_neg());
    }

    pub fn add_property(
        &mut self,
        id: i32,
        count: i32,
        catalog: &GameplayCatalog,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            catalog.properties.contains_key(&id),
            "unknown PAL4 property {id}"
        );
        let quantity = self.equipment_count(id).saturating_add(count).clamp(0, 99);
        if quantity == 0 {
            self.inventory.remove(&id);
        } else {
            self.inventory.insert(id, quantity);
        }
        Ok(())
    }

    pub fn has_equipment(&self, equip_id: i32) -> bool {
        self.inventory.get(&equip_id).copied().unwrap_or(0) > 0
    }

    pub fn equipment_count(&self, equip_id: i32) -> i32 {
        self.inventory.get(&equip_id).copied().unwrap_or(0)
    }

    /// Read-only iterator over `(equipment_id, count)` pairs in the
    /// inventory. Order is unspecified — callers that need a stable
    /// layout (e.g. for `/v1/state` snapshots) should sort.
    pub fn inventory_iter(&self) -> impl Iterator<Item = (&i32, &i32)> {
        self.inventory.iter()
    }

    // --- Scene / leader / position ------------------------------------

    pub fn leader(&self) -> usize {
        self.leader
    }

    pub fn set_leader(&mut self, leader: usize) {
        self.leader = leader;
    }

    pub fn scene_name(&self) -> &str {
        &self.scene_name
    }

    pub fn block_name(&self) -> &str {
        &self.block_name
    }

    pub fn sub_block_name(&self) -> &str {
        &self.sub_block_name
    }

    pub fn set_scene(&mut self, scene_name: String, block_name: String) {
        self.scene_name = scene_name;
        self.block_name = block_name;
        self.sub_block_name = String::new();
    }

    /// Same as [`Self::set_scene`] but records the gameplay-data
    /// sub-block so a later reload re-enters the same block variant.
    pub fn set_scene_with_sub_block(
        &mut self,
        scene_name: String,
        block_name: String,
        sub_block_name: String,
    ) {
        self.scene_name = scene_name;
        self.block_name = block_name;
        self.sub_block_name = sub_block_name;
    }

    pub fn position(&self) -> Option<Vec3> {
        self.position
    }

    pub fn set_position(&mut self, position: Option<Vec3>) {
        self.position = position;
    }

    pub fn direction(&self) -> Option<f32> {
        self.direction
    }

    pub fn set_direction(&mut self, direction: Option<f32>) {
        self.direction = direction;
    }

    pub fn player_locked(&self) -> bool {
        self.player_locked
    }

    pub fn set_player_locked(&mut self, locked: bool) {
        self.player_locked = locked;
    }

    pub fn camera(&self) -> Option<&Transform> {
        self.camera.as_ref()
    }

    pub fn set_camera(&mut self, camera: Option<Transform>) {
        self.camera = camera;
    }

    // --- Script globals (story plot) ----------------------------------

    pub fn script_globals(&self) -> &[u32] {
        &self.script_globals
    }

    pub fn set_script_globals(&mut self, globals: Vec<u32>) {
        self.script_globals = globals;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openpal4::gameplay::{
        EquipmentDefinition, EquipmentType, LevelDefinition, RoleDefinition,
    };

    fn catalog() -> GameplayCatalog {
        let mut catalog = GameplayCatalog::default();
        let stats = |hp, mp, physical| CharacterStats {
            integers: [
                ("CA_MAX_HP".to_string(), hp),
                ("CA_MAX_MP".to_string(), mp),
                ("CA_PHYSICAL".to_string(), physical),
                ("EFE_WATER".to_string(), 0),
            ]
            .into(),
            ..Default::default()
        };
        for slot in 0..PLAYER_COUNT {
            catalog.roles.insert(
                slot,
                RoleDefinition {
                    id: 1001 + slot as i32,
                    name: format!("role {slot}"),
                    model: String::new(),
                    stats: stats(90, 30, 18),
                },
            );
            for (level, hp, mp, physical, experience) in
                [(1, 210, 30, 24, 54), (2, 250, 40, 30, 150)]
            {
                catalog.levels.insert(
                    (slot, level),
                    LevelDefinition {
                        experience,
                        stats: stats(hp, mp, physical),
                    },
                );
            }
        }
        for (id, kind, hp, physical) in [
            (3501, EquipmentType::Sword, 0, 10),
            (3502, EquipmentType::Sword, 0, 20),
            (3671, EquipmentType::Accessory, 50, 0),
            (3672, EquipmentType::Accessory, 20, 0),
        ] {
            catalog.equipment.insert(
                id,
                EquipmentDefinition {
                    id,
                    kind,
                    name: String::new(),
                    description: String::new(),
                    model: String::new(),
                    texture: String::new(),
                    level_limits: [1, 0, 0, 1],
                    price: 0,
                    skill_id: 0,
                    stats: stats(hp, 0, physical),
                },
            );
        }
        catalog
    }

    #[test]
    fn progression_replaces_role_stats_and_refills_without_losing_affinity_or_equipment() {
        let catalog = catalog();
        let mut state = Pal4PersistentState::new(PAL4_APP_NAME.to_string());
        state.initialize_players(&catalog).unwrap();
        assert_eq!(
            (state.player(0).unwrap().hp, state.player(0).unwrap().mp),
            (210, 30)
        );
        assert!(state.player(0).unwrap().in_team);
        assert!(!state.player(1).unwrap().in_team);
        state.add_player_equip(0, 3501, &catalog).unwrap();
        let player = state.player_mut(0);
        player
            .base_stats
            .integers
            .insert("EFE_WATER".to_string(), 12);
        player.hp = 1;
        player.mp = 2;
        state.set_player_level(0, 2, &catalog).unwrap();
        let player = state.player(0).unwrap();
        assert_eq!(
            (player.hp, player.max_hp, player.mp, player.experience),
            (250, 250, 40, 54)
        );
        assert_eq!(player.effective_stats.integer("CA_PHYSICAL"), 40);
        assert_eq!(player.effective_stats.integer("EFE_WATER"), 12);
        assert!(state.set_player_level(0, 100, &catalog).is_err());
        assert!(state.set_player_level(usize::MAX, 1, &catalog).is_err());
        assert_eq!(state.player(0).unwrap().level, 2);
    }

    #[test]
    fn script_equipment_uses_six_slots_without_consuming_backpack_copies() {
        let catalog = catalog();
        let mut state = Pal4PersistentState::new(PAL4_APP_NAME.to_string());
        state.initialize_players(&catalog).unwrap();
        state.add_equipment(3501, 2);
        state.add_player_equip(0, 3501, &catalog).unwrap();
        state.add_player_equip(0, 3502, &catalog).unwrap();
        state.add_player_equip(0, 3671, &catalog).unwrap();
        state.add_player_equip(0, 3671, &catalog).unwrap();
        let player = state.player(0).unwrap();
        assert_eq!(player.equipment.weapon, Some(3502));
        assert_eq!(player.equipment.accessories, [Some(3671), Some(3671)]);
        assert_eq!(player.effective_stats.integer("CA_PHYSICAL"), 44);
        assert_eq!((player.hp, player.max_hp), (210, 310));
        assert_eq!(state.equipment_count(3501), 3);
        state.add_player_equip(0, 3672, &catalog).unwrap();
        assert_eq!(
            state.player(0).unwrap().equipment.accessories,
            [Some(3672), Some(3671)]
        );
        assert_eq!(state.player(0).unwrap().max_hp, 280);
        assert!(state.add_player_equip(0, 123, &catalog).is_err());
        let restored: Pal4PersistentState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(
            restored.player(0).unwrap().equipment,
            state.player(0).unwrap().equipment
        );
        assert_eq!(
            restored.player(0).unwrap().effective_stats,
            state.player(0).unwrap().effective_stats
        );
        assert_eq!(restored.equipment_count(3501), 3);
    }

    #[test]
    fn percentages_target_all_characters_but_full_restoration_targets_only_party() {
        let catalog = catalog();
        let mut state = Pal4PersistentState::new(PAL4_APP_NAME.to_string());
        state.initialize_players(&catalog).unwrap();
        state.set_in_team(1, true);
        state.set_player_resource_percent(-1, -1, 50);
        for slot in 0..PLAYER_COUNT {
            let player = state.player(slot).unwrap();
            assert_eq!((player.hp, player.mp), (105, 15));
        }
        state.restore_party_resources(true);
        state.restore_party_resources(false);
        assert_eq!(
            (state.player(1).unwrap().hp, state.player(1).unwrap().mp),
            (210, 30)
        );
        assert_eq!(
            (state.player(2).unwrap().hp, state.player(2).unwrap().mp),
            (105, 15)
        );
        state.set_player_resource_percent(2, 0, 200);
        state.set_player_resource_percent(2, 1, -10);
        assert_eq!(
            (state.player(2).unwrap().hp, state.player(2).unwrap().mp),
            (210, 0)
        );
        let before = serde_json::to_string(&state.player(2)).unwrap();
        state.set_player_resource_percent(2, 9, 0);
        state.set_player_resource_percent(99, -1, 0);
        state.set_in_team(99, true);
        assert_eq!(serde_json::to_string(&state.player(2)).unwrap(), before);
        assert!(state.player(99).is_none());
    }

    #[test]
    fn inventory_arithmetic_cannot_overflow_or_create_negative_stacks() {
        let mut state = Pal4PersistentState::new(PAL4_APP_NAME.to_string());
        state.add_equipment(3501, i32::MAX);
        state.add_equipment(3501, 1);
        assert_eq!(state.equipment_count(3501), i32::MAX);
        state.remove_equipment(3501, i32::MAX);
        assert!(!state.has_equipment(3501));
        assert!(state.inventory_iter().next().is_none());
    }

    #[test]
    fn backpack_equipping_checks_eligibility_and_swaps_owned_copies_atomically() {
        let mut catalog = catalog();
        catalog.equipment.get_mut(&3502).unwrap().level_limits[0] = 2;
        let mut state = Pal4PersistentState::new(PAL4_APP_NAME.to_string());
        state.initialize_players(&catalog).unwrap();
        state.add_player_equip(0, 3501, &catalog).unwrap();
        state.add_equipment(3502, 1);
        assert!(state.equip_from_inventory(0, 3502, &catalog).is_err());
        assert!(state.equip_from_inventory(1, 3502, &catalog).is_err());
        assert!(state.equip_from_inventory(99, 3502, &catalog).is_err());
        assert_eq!(state.equipment_count(3502), 1);
        assert_eq!(state.player(0).unwrap().equipment.weapon, Some(3501));

        state.set_player_level(0, 2, &catalog).unwrap();
        state.equip_from_inventory(0, 3502, &catalog).unwrap();
        assert_eq!(state.player(0).unwrap().equipment.weapon, Some(3502));
        assert_eq!(state.equipment_count(3501), 1);
        assert_eq!(state.equipment_count(3502), 0);
        assert_eq!(
            state
                .player(0)
                .unwrap()
                .effective_stats
                .integer("CA_PHYSICAL"),
            50
        );
        assert!(state.equip_from_inventory(0, 3502, &catalog).is_err());

        state.add_equipment(3501, 1);
        state.add_equipment(3502, i32::MAX);
        let before = serde_json::to_string(state.player(0).unwrap()).unwrap();
        assert!(state.equip_from_inventory(0, 3501, &catalog).is_err());
        assert_eq!(
            serde_json::to_string(state.player(0).unwrap()).unwrap(),
            before
        );
        assert_eq!(state.equipment_count(3501), 2);
        assert_eq!(state.equipment_count(3502), i32::MAX);
    }

    #[test]
    fn summary_with_scene_and_block() {
        let mut state = Pal4PersistentState::new("OpenPAL4".to_string());
        state.set_scene("m01".to_string(), "1".to_string());
        state.add_quest_percentage(42);
        assert_eq!(state.summary(), "m01/1 - Quest 42%");
    }

    #[test]
    fn summary_with_scene_no_block() {
        let mut state = Pal4PersistentState::new("OpenPAL4".to_string());
        state.set_scene("m01".to_string(), String::new());
        assert_eq!(state.summary(), "m01 - Quest 0%");
    }

    #[test]
    fn summary_without_scene() {
        let state = Pal4PersistentState::new("OpenPAL4".to_string());
        assert_eq!(state.summary(), "Quest 0%");
    }

    #[test]
    fn camera_transform_survives_json_round_trip() {
        let mut transform = Transform::new();
        transform
            .set_position(&Vec3::new(12.0, 34.0, 56.0))
            .look_at(&Vec3::new(0.0, 5.0, 0.0));

        let mut state = Pal4PersistentState::new("OpenPAL4".to_string());
        state.set_camera(Some(transform.clone()));

        let json = serde_json::to_string(&state).unwrap();
        let restored: Pal4PersistentState = serde_json::from_str(&json).unwrap();

        let restored_cam = restored.camera().expect("camera persisted");
        // The full 4x4 matrix (position + orientation) must match
        // exactly so the loaded view is identical to the saved one.
        let a = restored_cam.matrix();
        let b = transform.matrix();
        for r in 0..4 {
            for c in 0..4 {
                assert_eq!(a[r][c], b[r][c], "matrix mismatch at [{}][{}]", r, c);
            }
        }
    }

    #[test]
    fn camera_defaults_to_none_for_legacy_saves() {
        // A save JSON without the `camera` field (older format) must
        // still deserialize, leaving the camera unset so the loaded
        // scene keeps its default view.
        let json = r#"{"app_name":"OpenPAL4","scene_name":"m01","block_name":"1"}"#;
        let state: Pal4PersistentState = serde_json::from_str(json).unwrap();
        assert!(state.camera().is_none());
    }

    #[test]
    fn new_game_starts_player_locked() {
        // A fresh playthrough must start control-locked so the new-game
        // opening cutscene runs before the player gets control. This is
        // also the value `giNewGame` resets to (it calls `new`).
        let state = Pal4PersistentState::new("OpenPAL4".to_string());
        assert!(state.player_locked());
    }

    #[test]
    fn legacy_save_without_player_locked_deserializes_unlocked() {
        // The `new()` default (locked) must NOT leak into deserialization:
        // older saves that predate the `player_locked` field were taken
        // during free movement, so the serde default keeps them
        // controllable (`false`) after load.
        let json = r#"{"app_name":"OpenPAL4","scene_name":"m01","block_name":"1"}"#;
        let state: Pal4PersistentState = serde_json::from_str(json).unwrap();
        assert!(!state.player_locked());
    }
}
