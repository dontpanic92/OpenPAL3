use std::path::Path;

use crosscom::ComRc;

use super::{
    comdef::{IPal3SaveSlots, IPal3SaveSlotsImpl},
    states::persistent_state::PersistentState,
};

pub const SAVE_SLOT_COUNT: i32 = 36;

#[derive(Default)]
struct Slot {
    status: i32,
    scene: String,
    seconds: i32,
    money: i32,
    members: Vec<i32>,
    levels: [i32; 5],
}

pub struct Pal3SaveSlots {
    slots: Vec<Slot>,
}

ComObject_Pal3SaveSlots!(super::Pal3SaveSlots);

impl Pal3SaveSlots {
    pub fn snapshot(app_name: &str) -> ComRc<IPal3SaveSlots> {
        Self::snapshot_in(
            app_name,
            &crate::ydirs::save_dir().join(app_name).join("Save"),
        )
    }

    fn snapshot_in(app_name: &str, directory: &Path) -> ComRc<IPal3SaveSlots> {
        let slots = (1..=SAVE_SLOT_COUNT)
            .map(|index| {
                let path = directory.join(format!("{index}.json"));
                match PersistentState::load_from_path(app_name, &path) {
                    Ok(state) => Slot {
                        status: 1,
                        scene: format!(
                            "{} / {}",
                            state.scene_name().unwrap(),
                            state.sub_scene_name().unwrap()
                        ),
                        seconds: state.play_seconds(),
                        money: state.money(),
                        members: state.party().members().collect(),
                        levels: std::array::from_fn(|role| {
                            state
                                .stats()
                                .character(role as i32)
                                .map_or(0, |c| c.level())
                        }),
                    },
                    Err(_) => Slot {
                        status: if path.exists() { 2 } else { 0 },
                        ..Slot::default()
                    },
                }
            })
            .collect();
        ComRc::from_object(Self { slots })
    }

    fn slot(&self, slot: i32) -> Option<&Slot> {
        slot.checked_sub(1).and_then(|i| self.slots.get(i as usize))
    }
}

impl IPal3SaveSlotsImpl for Pal3SaveSlots {
    fn slot_count(&self) -> i32 {
        SAVE_SLOT_COUNT
    }
    fn status(&self, slot: i32) -> i32 {
        self.slot(slot).map_or(0, |s| s.status)
    }
    fn scene(&self, slot: i32) -> &str {
        self.slot(slot).map_or("", |s| &s.scene)
    }
    fn play_seconds(&self, slot: i32) -> i32 {
        self.slot(slot).map_or(0, |s| s.seconds)
    }
    fn money(&self, slot: i32) -> i32 {
        self.slot(slot).map_or(0, |s| s.money)
    }
    fn member_role(&self, slot: i32, index: i32) -> i32 {
        self.slot(slot)
            .and_then(|s| s.members.get(index as usize))
            .copied()
            .unwrap_or(-1)
    }

    fn role_level(&self, slot: i32, role: i32) -> i32 {
        self.slot(slot)
            .and_then(|s| s.levels.get(role as usize))
            .copied()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_snapshots_distinguish_empty_corrupt_and_complete_slots_and_refresh_after_overwrite() {
        let directory = std::env::temp_dir().join(format!(
            "pal3-save-browser-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("36.json");
        let mut state = PersistentState::new("ignored-save-app".into());
        state.set_scene_name("YZZ".into(), "AN".into());
        state.party_mut().set_in_team(1, true);
        state.add_money(321);
        state.advance_play_time(3661.0);
        state.save_to_path(&path).unwrap();
        std::fs::write(directory.join("1.json"), b"{broken").unwrap();
        std::fs::write(
            directory.join("2.json"),
            serde_json::to_string(&PersistentState::new("test".into())).unwrap(),
        )
        .unwrap();

        let snapshot = Pal3SaveSlots::snapshot_in("test", &directory);
        assert_eq!(snapshot.status(1), 2);
        assert_eq!(snapshot.status(2), 2);
        assert_eq!(snapshot.status(3), 0);
        assert_eq!(snapshot.status(36), 1);
        assert_eq!(snapshot.scene(36), "YZZ / AN");
        assert_eq!(snapshot.play_seconds(36), 3661);
        assert_eq!(snapshot.member_role(36, 1), 1);
        assert_eq!(snapshot.member_role(36, -1), -1);
        assert_eq!(
            snapshot.role_level(36, 0),
            state.stats().character(0).unwrap().level()
        );
        assert_eq!(snapshot.money(36), 321);

        state.add_money(10);
        state.save_to_path(&path).unwrap();
        assert_eq!(snapshot.money(36), 321);
        assert_eq!(
            Pal3SaveSlots::snapshot_in("test", &directory).money(36),
            331
        );
        let loaded = PersistentState::load_from_path("test", &path).unwrap();
        assert_eq!(loaded.app_name(), "test");
        assert_eq!(loaded.money(), 331);
        assert_eq!(loaded.party().members().collect::<Vec<_>>(), vec![0, 1]);
        assert!(PersistentState::load_from_path("test", &directory.join("1.json")).is_err());

        let blocked = directory.join("blocked.json");
        std::fs::create_dir(&blocked).unwrap();
        assert!(state.save_to_path(&blocked).is_err());
        assert!(blocked.is_dir());
        assert!(
            !blocked
                .with_extension(format!("json.{}.tmp", std::process::id()))
                .exists()
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
