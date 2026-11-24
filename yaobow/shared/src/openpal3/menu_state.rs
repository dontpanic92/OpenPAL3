use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crosscom::ComRc;

use super::{
    comdef::{IPal3MenuState, IPal3MenuStateImpl},
    loaders::stats::StatsData,
    states::persistent_state::PersistentState,
};

pub struct Pal3MenuState {
    state: Rc<RefCell<PersistentState>>,
    data: Option<Rc<StatsData>>,
    descriptions: HashMap<u32, String>,
}

ComObject_Pal3MenuState!(super::Pal3MenuState);

impl Pal3MenuState {
    pub fn create(
        state: Rc<RefCell<PersistentState>>,
        data: Option<Rc<StatsData>>,
    ) -> ComRc<IPal3MenuState> {
        let descriptions = data
            .iter()
            .flat_map(|data| data.items())
            .map(|item| (item.id, wrap_description(&item.description)))
            .collect();
        ComRc::from_object(Self {
            state,
            data,
            descriptions,
        })
    }
}

impl IPal3MenuStateImpl for Pal3MenuState {
    fn available(&self) -> bool {
        self.data.is_some()
    }

    fn controlled_role(&self) -> i32 {
        self.state.borrow().role_controlled()
    }

    fn member_count(&self) -> i32 {
        self.state.borrow().party().members().count() as i32
    }

    fn member_role(&self, index: i32) -> i32 {
        self.state
            .borrow()
            .party()
            .members()
            .nth(index as usize)
            .unwrap_or(-1)
    }

    fn money(&self) -> i32 {
        self.state.borrow().money()
    }

    fn play_seconds(&self) -> i32 {
        self.state.borrow().play_seconds()
    }

    fn role_value(&self, role: i32, field: i32) -> i32 {
        let Some(data) = self.data.as_deref() else {
            return 0;
        };
        let state = self.state.borrow();
        let Some(character) = state.stats().character(role) else {
            return 0;
        };
        let attributes = state.stats().attributes(role, data).unwrap();
        let current = character.current();
        match field {
            0 => character.level(),
            1 => character.experience(),
            2 if character.level() < 99 => data
                .experience_threshold(character.level() + 1)
                .saturating_sub(character.experience())
                .max(0),
            3 => current.hp,
            4 => current.qi,
            5 => current.mp,
            6 => attributes.hp,
            7 => attributes.qi,
            8 => attributes.mp,
            9 => attributes.attack,
            10 => attributes.defense,
            11 => attributes.speed,
            12 => attributes.luck,
            13..=17 => attributes.elements[(field - 13) as usize],
            _ => 0,
        }
    }

    fn equipment_item(&self, role: i32, slot: i32) -> i32 {
        self.state
            .borrow()
            .stats()
            .character(role)
            .and_then(|character| character.equipment().get(slot as usize))
            .copied()
            .flatten()
            .unwrap_or(0) as i32
    }

    fn inventory_count(&self) -> i32 {
        self.state.borrow().inventory().stacks().len() as i32
    }

    fn inventory_item(&self, index: i32) -> i32 {
        self.state
            .borrow()
            .inventory()
            .stacks()
            .get(index as usize)
            .map_or(0, |stack| stack.id as i32)
    }

    fn inventory_quantity(&self, index: i32) -> i32 {
        self.state
            .borrow()
            .inventory()
            .stacks()
            .get(index as usize)
            .map_or(0, |stack| stack.count)
    }

    fn item_name(&self, id: i32) -> &str {
        self.data
            .as_ref()
            .and_then(|data| data.item(id as u32))
            .map_or("", |item| item.name.as_str())
    }

    fn item_description(&self, id: i32) -> &str {
        self.descriptions
            .get(&(id as u32))
            .map_or("", String::as_str)
    }

    fn item_price(&self, id: i32) -> i32 {
        self.data
            .as_ref()
            .and_then(|data| data.item(id as u32))
            .map_or(0, |item| item.price)
    }
}

fn wrap_description(text: &str) -> String {
    let mut output = String::new();
    let mut columns = 0;
    for ch in text.chars().filter(|&ch| ch != '$' && ch != '\r') {
        let width = if ch.is_ascii() { 1 } else { 2 };
        if ch == '\n' {
            columns = 0;
        } else {
            if columns + width > 64 {
                output.push('\n');
                columns = 0;
            }
            columns += width;
        }
        output.push(ch);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openpal3::{loaders::stats::test_data, states::stats::Resource};

    #[test]
    fn unavailable_service_never_requires_the_pal3_database() {
        let ui = Pal3MenuState::create(
            Rc::new(RefCell::new(PersistentState::new("test".into()))),
            None,
        );
        assert!(!ui.available());
        assert_eq!(ui.role_value(0, 6), 0);
        assert_eq!(ui.item_name(82201), "");
        assert_eq!(ui.item_description(82201), "");
    }

    #[test]
    fn description_runs_are_plain_text_wrapped_without_splitting_unicode() {
        assert_eq!(wrap_description("$test\r\n$next"), "test\nnext");
        let text = format!("{}abc", "水".repeat(32));
        assert_eq!(wrap_description(&text), format!("{}\nabc", "水".repeat(32)));
    }

    #[test]
    fn service_reads_live_membership_resources_equipment_and_bag_without_mutating() {
        let data = Rc::new(test_data());
        let state = Rc::new(RefCell::new(PersistentState::new("test".into())));
        let ui = Pal3MenuState::create(state.clone(), Some(data.clone()));
        assert_eq!(ui.member_count(), 1);
        assert_eq!(ui.member_role(0), 0);
        assert_eq!(ui.member_role(-1), -1);
        {
            let mut state = state.borrow_mut();
            state.party_mut().set_in_team(3, true);
            state.set_role_controlled(3);
            state.add_money(321);
            state.stats_mut().set_resource(3, Resource::Hp, 7, &data);
            state.add_item(42, -2, &data);
            state.add_item(6008, 0, &data);
        }
        let before = serde_json::to_string(&*state.borrow()).unwrap();
        assert_eq!(ui.member_count(), 2);
        assert_eq!(ui.member_role(1), 3);
        assert_eq!(ui.controlled_role(), 3);
        assert_eq!(ui.money(), 321);
        assert_eq!(ui.role_value(3, 3), 7);
        assert_eq!(
            ui.role_value(3, 6),
            state.borrow().stats().attributes(3, &data).unwrap().hp
        );
        assert_eq!(ui.equipment_item(1, 5), 82201);
        assert_eq!(ui.inventory_count(), 2);
        assert_eq!(ui.inventory_item(0), 42);
        assert_eq!(ui.inventory_quantity(0), -2);
        assert_eq!(ui.inventory_item(1), 6008);
        assert_eq!(ui.inventory_quantity(1), 0);
        assert_eq!(serde_json::to_string(&*state.borrow()).unwrap(), before);
    }

    #[test]
    fn invalid_indices_and_missing_metadata_are_safe() {
        let ui = Pal3MenuState::create(
            Rc::new(RefCell::new(PersistentState::new("test".into()))),
            Some(Rc::new(test_data())),
        );
        for index in [-1, 5, i32::MAX] {
            assert_eq!(ui.member_role(index), -1);
            assert_eq!(ui.role_value(index, 6), 0);
            assert_eq!(ui.equipment_item(index, 5), 0);
            assert_eq!(ui.inventory_item(index), 0);
            assert_eq!(ui.inventory_quantity(index), 0);
        }
        assert_eq!(ui.role_value(0, -1), 0);
        assert_eq!(ui.role_value(0, 18), 0);
        assert_eq!(ui.equipment_item(0, -1), 0);
        assert_eq!(ui.item_name(-1), "");
        assert_eq!(ui.item_description(-1), "");
        assert_eq!(ui.item_price(-1), 0);
    }
}
