use fileformats::pal3::gdb::ItemDefinition;
use serde::{Deserialize, Serialize};

use crate::openpal3::loaders::stats::StatsData;

use super::party::PartyState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resource {
    Hp,
    Qi,
    Mp,
}

impl TryFrom<i32> for Resource {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Hp),
            1 => Ok(Self::Qi),
            2 => Ok(Self::Mp),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resources {
    pub hp: i32,
    pub qi: i32,
    pub mp: i32,
}

impl Resources {
    fn get_mut(&mut self, resource: Resource) -> &mut i32 {
        match resource {
            Resource::Hp => &mut self.hp,
            Resource::Qi => &mut self.qi,
            Resource::Mp => &mut self.mp,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attributes {
    pub hp: i32,
    pub qi: i32,
    pub mp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub luck: i32,
    /// Water, fire, wind, thunder, earth.
    pub elements: [i32; 5],
}

impl Attributes {
    fn values(self) -> [i32; 12] {
        [
            self.hp,
            self.qi,
            self.mp,
            self.attack,
            self.defense,
            self.speed,
            self.luck,
            self.elements[0],
            self.elements[1],
            self.elements[2],
            self.elements[3],
            self.elements[4],
        ]
    }

    fn from_values(values: [i32; 12]) -> Self {
        Self {
            hp: values[0],
            qi: values[1],
            mp: values[2],
            attack: values[3],
            defense: values[4],
            speed: values[5],
            luck: values[6],
            elements: values[7..].try_into().unwrap(),
        }
    }

    fn resources(self) -> Resources {
        Resources {
            hp: self.hp,
            qi: self.qi,
            mp: self.mp,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterStats {
    level: i32,
    experience: i32,
    base: Attributes,
    current: Resources,
    equipment: [Option<u32>; 6],
    /// Native poison order: wind, thunder, water, fire, earth.
    poisons: [u8; 5],
}

impl CharacterStats {
    fn new(role: usize) -> Self {
        let level = [1, 1, 14, 10, 13][role];
        let grown = growth(role, level);
        let base = Attributes {
            hp: grown[0],
            qi: grown[1],
            mp: grown[2],
            attack: grown[3],
            defense: grown[4],
            speed: [3, 5, 4, 4, 3][role],
            luck: [2, 4, 5, 1, 3][role],
            elements: [
                [3, 3, 6, 1, 2],
                [2, 4, 2, 3, 5],
                [4, 2, 3, 6, 5],
                [6, 3, 4, 3, 2],
                [1, 4, 3, 2, 2],
            ][role],
        };
        let mut equipment = [None; 6];
        equipment[5] = [None, Some(82201), Some(82101), Some(82301), Some(82001)][role];
        Self {
            level,
            experience: [0, 0, 5594, 1943, 4341][role],
            base,
            current: base.resources(),
            equipment,
            poisons: [0; 5],
        }
    }

    pub fn level(&self) -> i32 {
        self.level
    }

    pub fn experience(&self) -> i32 {
        self.experience
    }

    pub fn base(&self) -> Attributes {
        self.base
    }

    pub fn current(&self) -> Resources {
        self.current
    }

    /// Native equipment slots; the weapon occupies slot 5.
    pub fn equipment(&self) -> &[Option<u32>; 6] {
        &self.equipment
    }

    pub fn poisons(&self) -> &[u8; 5] {
        &self.poisons
    }

    fn attributes(&self, data: &StatsData, magic_sword: &MagicSwordStats) -> Attributes {
        let mut values = self.base.values();
        // Native 0x40be80: weapon modifiers only use the first seven amounts.
        for (slot, id) in self.equipment.iter().enumerate() {
            let Some(item) = id.and_then(|id| data.item(id)) else {
                continue;
            };
            for (index, value) in values.iter_mut().enumerate() {
                if item.modes[index] == 0 && (slot != 5 || index < 7) {
                    *value = value.wrapping_add(item.amounts[index] as i32);
                }
                if index >= 7 && item.elements[index - 7] {
                    *value = value.wrapping_add(1);
                }
            }
            if slot == 5 && item.id == 6603 {
                values[3] = values[3].wrapping_add(magic_sword.attack_bonus);
                if let Some(&element) = [0, 1, 3, 2, 4].get(magic_sword.element as usize) {
                    values[7 + element] = values[7 + element].wrapping_add(1);
                }
            }
        }
        Attributes::from_values(values)
    }

    fn clamp_resources(&mut self, data: &StatsData, magic_sword: &MagicSwordStats) {
        let max = self.attributes(data, magic_sword).resources();
        self.current.hp = self.current.hp.clamp(0, max.hp.max(0));
        self.current.qi = self.current.qi.clamp(0, max.qi.max(0));
        self.current.mp = self.current.mp.clamp(0, max.mp.max(0));
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MagicSwordStats {
    attack_bonus: i32,
    element: i32,
}

impl Default for MagicSwordStats {
    fn default() -> Self {
        // Native reset 0x4078b8 / 0x4078dc seeds wind and +35 attack.
        Self {
            attack_bonus: 35,
            element: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsState {
    characters: [CharacterStats; 5],
    magic_sword: MagicSwordStats,
}

impl Default for StatsState {
    fn default() -> Self {
        Self {
            characters: std::array::from_fn(CharacterStats::new),
            magic_sword: MagicSwordStats::default(),
        }
    }
}

impl StatsState {
    pub fn hazard_damage(
        &mut self,
        party: &PartyState,
        fraction: f32,
        data: &StatsData,
    ) -> [i32; 5] {
        let mut deltas = [0; 5];
        for role in party.members() {
            let character = &mut self.characters[role as usize];
            let maximum = character.attributes(data, &self.magic_sword).hp;
            let old = character.current.hp;
            // 0x4101f0 keeps the subtraction in x87 precision; 0x553514
            // truncates the FINAL value. 100 HP with native 0.1f becomes 89.
            let hp = ((old as f64 - maximum as f64 * fraction as f64).trunc() as i32).max(1);
            character.current.hp = hp;
            deltas[role as usize] = hp - old;
        }
        deltas
    }

    pub(crate) fn set_equipment(
        &mut self,
        role: i32,
        slot: usize,
        id: Option<u32>,
        data: &StatsData,
    ) {
        let character = &mut self.characters[role as usize];
        character.equipment[slot] = id;
        character.clamp_resources(data, &self.magic_sword);
    }

    /// Poison indices use native wind/thunder/water/fire/earth order.
    pub fn set_poison(&mut self, role: i32, element: usize, value: u8) -> bool {
        let Some(poison) = self
            .characters
            .get_mut(role as usize)
            .and_then(|character| character.poisons.get_mut(element))
        else {
            return false;
        };
        *poison = value;
        true
    }

    pub(crate) fn item_applies(&self, role: i32, item: &ItemDefinition, data: &StatsData) -> bool {
        let Some(character) = self.character(role) else {
            return false;
        };
        if item.use_kind == 15 {
            return true;
        }
        let maximum = character.attributes(data, &self.magic_sword).values();
        let mut current = maximum;
        current[..3].copy_from_slice(&[
            character.current.hp,
            character.current.qi,
            character.current.mp,
        ]);
        // Native 0x49ca70 also admits zero-HP targets and permanent boosts at full pools.
        let mut score = 0i32;
        for i in 0..12 {
            if item.modes[i] == 3 {
                score = score.wrapping_add(1);
            }
            if item.amounts[i] > 0 {
                score = score.wrapping_add(maximum[i].wrapping_sub(current[i]));
            }
        }
        for i in 0..5 {
            if item.conditions[i] != 0 {
                score = score.wrapping_add(character.poisons[i] as i32);
            }
        }
        score > 0
    }

    pub(crate) fn apply_item(&mut self, role: i32, item: &ItemDefinition, data: &StatsData) {
        if item.use_kind == 15 {
            let character = &mut self.characters[role as usize];
            let old = growth(role as usize, character.level);
            character.level = character.level.wrapping_add(1).min(99);
            let new = growth(role as usize, character.level);
            let mut base = character.base.values();
            for i in 0..5 {
                base[i] = base[i].wrapping_sub(old[i]).wrapping_add(new[i]);
            }
            character.base = Attributes::from_values(base);
            character.clamp_resources(data, &self.magic_sword);
            return;
        }
        let character = &mut self.characters[role as usize];
        let maximum = character.attributes(data, &self.magic_sword).values();
        let mut base = character.base.values();
        for i in 0..12 {
            let amount = item.amounts[i] as i32;
            if item.modes[i] == 3 {
                base[i] = base[i].wrapping_add(amount);
            } else if i < 3 {
                let delta = if item.modes[i] == 1 {
                    // Native 0x49e040 stores f32 before rounding; 0x49e2c1
                    // divides party percentages before multiplying.
                    let value = if item.use_scope == 2 {
                        amount.wrapping_div(100).wrapping_mul(maximum[i]) as f32
                    } else {
                        (amount as f64 * 0.01f32 as f64 * maximum[i] as f64) as f32
                    }
                    .round_ties_even();
                    if value >= 2147483648.0 || value < -2147483648.0 {
                        i32::MIN
                    } else {
                        value as i32
                    }
                } else {
                    amount
                };
                let current = character
                    .current
                    .get_mut(Resource::try_from(i as i32).unwrap());
                *current = current.wrapping_add(delta);
            }
        }
        character.base = Attributes::from_values(base);
        for i in 0..5 {
            if item.conditions[i] >= 2 {
                character.poisons[i] = 0;
            }
        }
        character.clamp_resources(data, &self.magic_sword);
    }

    pub fn character(&self, role_id: i32) -> Option<&CharacterStats> {
        self.characters.get(role_id as usize)
    }

    pub fn attributes(&self, role_id: i32, data: &StatsData) -> Option<Attributes> {
        self.character(role_id)
            .map(|character| character.attributes(data, &self.magic_sword))
    }

    pub(crate) fn set_jingtian_weapon(&mut self, id: u32, data: &StatsData) {
        self.characters[0].equipment[5] = Some(id);
        self.characters[0].clamp_resources(data, &self.magic_sword);
    }

    pub(crate) fn remove_equipped_item(&mut self, id: u32, slots: std::ops::Range<usize>) {
        for character in &mut self.characters {
            for slot in &mut character.equipment[slots.clone()] {
                if *slot == Some(id) {
                    *slot = None;
                }
            }
        }
    }

    pub fn fill_all_resources(&mut self, data: &StatsData) {
        for character in &mut self.characters {
            character.current = character.attributes(data, &self.magic_sword).resources();
        }
    }

    pub fn set_resource(
        &mut self,
        role_id: i32,
        resource: Resource,
        value: i32,
        data: &StatsData,
    ) -> bool {
        let Some(character) = self.characters.get_mut(role_id as usize) else {
            return false;
        };
        *character.current.get_mut(resource) = value;
        character.clamp_resources(data, &self.magic_sword);
        true
    }

    pub fn restore_resource(&mut self, role_id: i32, resource: Resource, data: &StatsData) -> bool {
        let Some(character) = self.characters.get_mut(role_id as usize) else {
            return false;
        };
        let mut max = character.attributes(data, &self.magic_sword).resources();
        *character.current.get_mut(resource) = *max.get_mut(resource);
        true
    }

    pub fn add_base_attributes(
        &mut self,
        role_id: i32,
        bonus: Attributes,
        data: &StatsData,
    ) -> bool {
        let Some(character) = self.characters.get_mut(role_id as usize) else {
            return false;
        };
        let base = character.base.values();
        let bonus = bonus.values();
        character.base =
            Attributes::from_values(std::array::from_fn(|i| base[i].wrapping_add(bonus[i])));
        character.clamp_resources(data, &self.magic_sword);
        true
    }

    pub fn add_experience(&mut self, role_id: i32, amount: i32) -> bool {
        let Some(character) = self.characters.get_mut(role_id as usize) else {
            return false;
        };
        // Native EXP grants accumulate independently of level-up processing.
        character.experience = character.experience.wrapping_add(amount);
        true
    }

    pub fn advance_level(&mut self, role_id: i32, data: &StatsData) -> bool {
        let role = role_id as usize;
        let Some(character) = self.characters.get_mut(role) else {
            return false;
        };
        if character.level >= 99 {
            return false;
        }
        let old = growth(role, character.level);
        character.level += 1;
        let new = growth(role, character.level);
        let mut base = character.base.values();
        // Ordinary level-up preserves permanent bonuses and does not refill pools.
        for i in 0..5 {
            base[i] = base[i].wrapping_sub(old[i]).wrapping_add(new[i]);
        }
        character.base = Attributes::from_values(base);
        character.clamp_resources(data, &self.magic_sword);
        true
    }

    pub fn average_level(
        &mut self,
        party: &PartyState,
        role_id: i32,
        offset: i32,
        data: &StatsData,
    ) -> bool {
        let role = role_id as usize;
        if role >= self.characters.len() {
            return false;
        }
        let (sum, count) = party.members().filter(|&other| other != role_id).fold(
            (0i32, 0i32),
            |(sum, count), other| {
                (
                    sum.wrapping_add(self.characters[other as usize].level),
                    count + 1,
                )
            },
        );
        if count == 0 {
            return true;
        }
        let character = &mut self.characters[role];
        character.level = character.level.max((sum / count).wrapping_add(offset));
        character.experience = data.experience_threshold(character.level);
        let new = growth(role, character.level);
        let mut base = character.base.values();
        base[..5].copy_from_slice(&new);
        character.base = Attributes::from_values(base);
        character.current = character.attributes(data, &self.magic_sword).resources();
        true
    }
}

// Clean-room native growth: 0x408d10..0x408fb2, duplicated at 0x4fa210..0x4fa4c2.
fn growth(role: usize, level: i32) -> [i32; 5] {
    let linear = |base: i32, rate: i32| base.wrapping_add(level.wrapping_mul(rate));
    let l = f64::from(level as f32);
    let qi = if role == 4 {
        (l.sqrt() + 1.) * 9.
    } else {
        [8., 6., 7., 5.][role] + 9. * l.sqrt()
    };
    [
        linear([80, 75, 80, 60, 100][role], [8, 7, 9, 7, 9][role]),
        native_round(qi),
        linear([25, 25, 22, 30, 20][role], [4, 5, 4, 6, 4][role]),
        native_round([26., 24., 20., 22., 35.][role] + [4.5, 4., 4.5, 4., 5.][role] * l),
        native_round(
            [17., 13., 16., 16., 18.][role] + f64::from([3.3f32, 3., 3.5, 3., 3.][role]) * l,
        ),
    ]
}

fn native_round(value: f64) -> i32 {
    // Native x87 stores f32 before converting to i32 with nearest-even rounding.
    let rounded = (value as f32).round_ties_even() as f64;
    if !rounded.is_finite() || rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        i32::MIN
    } else {
        rounded as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openpal3::loaders::stats::test_data;

    #[test]
    fn hazard_damage_matches_native_final_truncation_party_filter_and_nonfatal_clamp() {
        let data = test_data();
        for (fraction, expected) in [(0.1, 89), (0.15, 84)] {
            let mut stats = StatsState::default();
            let mut party = PartyState::default();
            party.set_in_team(2, true);
            for character in &mut stats.characters {
                character.base.hp = 100;
                character.equipment = [None; 6];
                character.current = Resources {
                    hp: 100,
                    qi: 7,
                    mp: 11,
                };
            }
            stats.characters[2].current.hp = 0;
            let deltas = stats.hazard_damage(&party, fraction, &data);
            assert_eq!(deltas, [expected - 100, 0, 1, 0, 0]);
            assert_eq!(
                stats.characters[0].current,
                Resources {
                    hp: expected,
                    qi: 7,
                    mp: 11
                }
            );
            assert_eq!(stats.characters[1].current.hp, 100);
            assert_eq!(stats.characters[2].current.hp, 1);
            stats.hazard_damage(&party, fraction, &data);
            assert_eq!(stats.characters[2].current.hp, 1);
        }
        let mut stats = StatsState::default();
        let mut party = PartyState::default();
        party.set_in_team(1, true);
        stats.fill_all_resources(&data);
        stats.hazard_damage(&party, 0.15, &data);
        // Xuejian's equipment raises max HP from 82 to 87.
        assert_eq!(stats.characters[1].current.hp, 73);
    }
    #[test]
    fn equipped_item_cleanup_distinguishes_all_slots_from_linked_accessories() {
        let mut stats = StatsState::default();
        for role in &mut stats.characters {
            role.equipment = [Some(8150); 6];
        }
        stats.remove_equipped_item(8150, 3..5);
        for role in &stats.characters {
            assert_eq!(
                role.equipment,
                [Some(8150), Some(8150), Some(8150), None, None, Some(8150)]
            );
        }
        stats.remove_equipped_item(8150, 0..6);
        assert!(
            stats
                .characters
                .iter()
                .all(|role| role.equipment == [None; 6])
        );
    }

    #[test]
    fn native_seeds_and_growth_use_final_f32_and_ties_even_rounding() {
        let stats = StatsState::default();
        for (role, level, exp, expected) in [
            (0, 1, 0, [88, 17, 29, 30, 20]),
            (1, 1, 0, [82, 15, 30, 28, 16]),
            (2, 14, 5594, [206, 41, 78, 83, 65]),
            (3, 10, 1943, [130, 33, 90, 62, 46]),
            (4, 13, 4341, [217, 41, 72, 100, 57]),
        ] {
            let character = stats.character(role).unwrap();
            assert_eq!(character.level(), level);
            assert_eq!(character.experience(), exp);
            assert_eq!(character.base().values()[..5], expected);
        }
        assert_eq!(growth(0, 3)[3], 40);
        assert_eq!(growth(0, 5)[4], 34);
        assert_eq!(growth(0, 99), [872, 98, 421, 472, 344]);
    }

    #[test]
    fn equipment_uses_loaded_modifiers_and_keeps_current_pools_separate() {
        let data = test_data();
        let mut stats = StatsState::default();
        stats.fill_all_resources(&data);
        let character = stats.character(1).unwrap();
        let attributes = stats.attributes(1, &data).unwrap();
        assert_eq!(attributes.hp, 87);
        assert_eq!(attributes.attack, 35);
        assert_eq!(attributes.defense, 14);
        assert_eq!(attributes.mp, 30);
        assert_eq!(attributes.speed, 4);
        assert_eq!(attributes.elements, [2, 4, 2, 4, 5]);
        assert_eq!(character.base().attack, 28);
        assert_eq!(character.current().hp, 87);
        stats.set_resource(1, Resource::Hp, 200, &data);
        assert_eq!(stats.character(1).unwrap().current().hp, 87);
        stats.set_resource(1, Resource::Mp, -1, &data);
        assert_eq!(stats.character(1).unwrap().current().mp, 0);
        assert!(!stats.set_resource(5, Resource::Hp, 1, &data));
    }

    #[test]
    fn armor_adds_signed_element_amounts_and_flags_but_ignores_nonzero_modes() {
        let data = test_data();
        let mut stats = StatsState::default();
        stats.characters[1].equipment[0] = Some(99999);
        assert_eq!(
            stats.attributes(1, &data).unwrap(),
            Attributes {
                hp: 107,
                qi: 15,
                mp: 27,
                attack: 33,
                defense: 14,
                speed: 6,
                luck: 5,
                elements: [1, 4, 7, 4, 5],
            }
        );
    }

    #[test]
    fn ordinary_level_up_preserves_permanent_bonuses_and_injuries() {
        let data = test_data();
        let mut stats = StatsState::default();
        stats.set_resource(0, Resource::Hp, 12, &data);
        stats.add_base_attributes(
            0,
            Attributes {
                hp: 10,
                attack: 3,
                ..Default::default()
            },
            &data,
        );
        stats.add_experience(0, 123);
        assert!(stats.advance_level(0, &data));
        let character = stats.character(0).unwrap();
        assert_eq!(character.level(), 2);
        assert_eq!(character.base().hp, 106);
        assert_eq!(character.base().attack, 38);
        assert_eq!(character.current().hp, 12);
        assert_eq!(character.experience(), 123);
        while stats.advance_level(0, &data) {}
        assert_eq!(stats.character(0).unwrap().level(), 99);
        assert_eq!(stats.character(0).unwrap().base().hp, 882);
    }

    #[test]
    fn average_level_excludes_target_and_resets_even_when_level_is_unchanged() {
        let data = test_data();
        let mut stats = StatsState::default();
        let mut party = PartyState::default();
        party.set_in_team(3, true);
        assert!(stats.average_level(&party, 1, 2, &data));
        let character = stats.character(1).unwrap();
        assert_eq!(character.level(), 7);
        assert_eq!(character.experience(), 600);
        assert_eq!(character.current().hp, 129);

        party.set_in_team(1, true);
        stats.add_base_attributes(
            1,
            Attributes {
                hp: 99,
                ..Default::default()
            },
            &data,
        );
        stats.set_resource(1, Resource::Hp, 1, &data);
        stats.average_level(&party, 1, -50, &data);
        let character = stats.character(1).unwrap();
        assert_eq!(character.level(), 7);
        assert_eq!(character.base().hp, 124);
        assert_eq!(character.current().hp, 129);
        assert!(!stats.average_level(&party, -1, 0, &data));
    }

    #[test]
    fn average_level_with_no_other_members_is_a_noop_and_script_levels_are_not_capped() {
        let data = test_data();
        let mut stats = StatsState::default();
        let mut party = PartyState::default();
        stats.set_resource(0, Resource::Hp, 1, &data);
        let before = stats.clone();
        assert!(stats.average_level(&party, 0, 99, &data));
        assert_eq!(stats, before);
        party.set_in_team(3, true);
        stats.average_level(&party, 0, 100, &data);
        assert_eq!(stats.character(0).unwrap().level(), 110);
        assert_eq!(stats.character(0).unwrap().experience(), i32::MAX);
    }
}
