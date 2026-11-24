use serde::{Deserialize, Serialize};

const ROLE_COUNT: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyState {
    members: [bool; ROLE_COUNT],
    formation_slots: [u8; ROLE_COUNT],
}

impl Default for PartyState {
    fn default() -> Self {
        // Native new-game initialization at PAL3unpacked.exe 0x4092da.
        Self {
            members: [true, false, false, false, false],
            formation_slots: [3, 5, 1, 2, 4],
        }
    }
}

impl PartyState {
    pub fn contains(&self, role_id: i32) -> bool {
        self.members.get(role_id as usize).copied().unwrap_or(false)
    }

    pub fn members(&self) -> impl Iterator<Item = i32> + '_ {
        self.members
            .iter()
            .enumerate()
            .filter_map(|(role, &member)| member.then_some(role as i32))
    }

    pub fn formation_slot(&self, role_id: i32) -> Option<u8> {
        self.formation_slots.get(role_id as usize).copied()
    }

    pub fn set_in_team(&mut self, role_id: i32, joined: bool) -> bool {
        let role = role_id as usize;
        if role >= ROLE_COUNT {
            return false;
        }

        // InTeam (0x4351a0) only allocates a slot on an absent -> present edge.
        if joined && !self.members[role] {
            for slot in 1..=6 {
                let occupied = self
                    .members
                    .iter()
                    .enumerate()
                    .any(|(other, &member)| member && self.formation_slots[other] == slot);
                if !occupied {
                    self.formation_slots[role] = slot;
                    break;
                }
            }
        }
        self.members[role] = joined;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joining_allocates_first_free_slot_without_reordering_members() {
        let mut party = PartyState::default();
        assert_eq!(party.members().collect::<Vec<_>>(), vec![0]);

        for role in [4, 2, 1, 3] {
            assert!(party.set_in_team(role, true));
        }
        assert_eq!(party.members().collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
        assert_eq!(party.formation_slots, [3, 4, 2, 5, 1]);
        assert!(party.set_in_team(4, true));
        assert_eq!(party.formation_slot(4), Some(1));
    }

    #[test]
    fn leaving_retains_slot_but_rejoining_allocates_it_again() {
        let mut party = PartyState::default();
        party.set_in_team(1, true);
        party.set_in_team(1, false);
        assert!(!party.contains(1));
        assert_eq!(party.formation_slot(1), Some(1));

        party.set_in_team(2, true);
        party.set_in_team(1, true);
        assert_eq!(party.formation_slot(2), Some(1));
        assert_eq!(party.formation_slot(1), Some(2));

        for role in [0, 1, 2] {
            party.set_in_team(role, false);
        }
        assert_eq!(party.members().count(), 0);
    }

    #[test]
    fn invalid_role_ids_do_not_change_membership_or_slots() {
        let mut party = PartyState::default();
        for role in [-1, 5, 6, i32::MIN, i32::MAX] {
            assert!(!party.set_in_team(role, true));
            assert!(!party.contains(role));
            assert_eq!(party.formation_slot(role), None);
        }
        assert_eq!(party.members().collect::<Vec<_>>(), vec![0]);
        assert_eq!(party.formation_slots, [3, 5, 1, 2, 4]);
    }
}
