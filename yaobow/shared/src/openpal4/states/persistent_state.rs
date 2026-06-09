use std::collections::HashMap;
use std::path::PathBuf;

use radiance::math::Transform;
use radiance::math::Vec3;
use serde::{Deserialize, Serialize};

use crate::ydirs;

/// Number of fixed party slots in PAL4 (YunTianhe / HanLingsha /
/// LiuMengli / MurongZiying). Mirrors the `Player` enum in
/// `openpal4::scene`.
pub const PLAYER_COUNT: usize = 4;

/// Per-player progression record. All fields default so older save
/// files (or partially populated states) still deserialize cleanly.
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
    pub equipment: Vec<i32>,
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
            equipment: Vec::new(),
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
    #[serde(default)]
    position: Option<Vec3>,
    /// Leader facing direction (degrees, yaw about world-up) at save
    /// time, restored alongside `position` so the player stands facing
    /// the same way. `None` in older saves.
    #[serde(default)]
    direction: Option<f32>,
    /// Whether player control was locked (cutscene) at save time.
    /// Defaults to `false` for older saves, which were always taken
    /// during free movement, so they remain controllable after load.
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
            position: None,
            camera: None,
            direction: None,
            player_locked: false,
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

    pub fn set_player_level(&mut self, slot: usize, level: i32) {
        self.player_mut(slot).level = level;
    }

    pub fn player_level(&self, slot: usize) -> i32 {
        self.players.get(&slot).map(|p| p.level).unwrap_or(1)
    }

    pub fn set_in_team(&mut self, slot: usize, in_team: bool) {
        self.player_mut(slot).in_team = in_team;
    }

    pub fn add_skill(&mut self, slot: usize, skill_id: i32) {
        let player = self.player_mut(slot);
        if !player.skills.contains(&skill_id) {
            player.skills.push(skill_id);
        }
    }

    /// Attach an equipment id to a specific player slot.
    pub fn add_player_equip(&mut self, slot: usize, equip_id: i32) {
        let player = self.player_mut(slot);
        if !player.equipment.contains(&equip_id) {
            player.equipment.push(equip_id);
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
        *entry = (*entry + count).max(0);
        if *entry == 0 {
            self.inventory.remove(&equip_id);
        }
    }

    pub fn remove_equipment(&mut self, equip_id: i32, count: i32) {
        self.add_equipment(equip_id, -count);
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

    pub fn set_scene(&mut self, scene_name: String, block_name: String) {
        self.scene_name = scene_name;
        self.block_name = block_name;
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
}
