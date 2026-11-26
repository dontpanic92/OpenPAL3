use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use crosscom::ComRc;

use super::{
    comdef::{IPal4MenuState, IPal4MenuStateImpl},
    gameplay::{EquipmentType, GameplayCatalog},
    session::Pal4Session,
};

pub struct Pal4MenuState {
    session: Rc<RefCell<Pal4Session>>,
    catalog: Rc<GameplayCatalog>,
    available: Cell<bool>,
    descriptions: HashMap<i32, String>,
    effects: HashMap<i32, String>,
    preview: RefCell<Option<super::menu_preview::Pal4MenuPreview>>,
}

ComObject_Pal4MenuState!(super::Pal4MenuState);

impl Pal4MenuState {
    pub fn create(session: Rc<RefCell<Pal4Session>>) -> ComRc<IPal4MenuState> {
        let catalog = session.borrow().catalog();
        let descriptions = catalog
            .properties
            .iter()
            .map(|(&id, item)| (id, wrap_description(&item.description)))
            .chain(
                catalog
                    .equipment
                    .iter()
                    .map(|(&id, item)| (id, wrap_description(&item.description))),
            )
            .collect();
        let effects = catalog
            .properties
            .iter()
            .map(|(&id, item)| (id, wrap_description(&item.detail)))
            .collect();
        ComRc::from_object(Self {
            session,
            catalog,
            descriptions,
            effects,
            preview: RefCell::new(None),
            available: Cell::new(false),
        })
    }

    pub fn set_available(&self, available: bool) {
        self.available.set(available);
    }

    pub(crate) fn set_preview(&self, preview: super::menu_preview::Pal4MenuPreview) {
        *self.preview.borrow_mut() = Some(preview);
    }

    pub(crate) fn update_preview(&self, tab: Option<i32>) {
        if let Some(preview) = self.preview.borrow_mut().as_mut() {
            preview.update(tab);
        }
    }

    fn action_allowed(&self, tab: i32) -> bool {
        let session = self.session.borrow();
        session.menu_tab() == Some(tab)
            && session.state().menu_enabled(tab)
            && (self.available.get() || session.menu_forced())
    }

    fn inventory_ids(&self, tab: i32, category: i32) -> Vec<i32> {
        let session = self.session.borrow();
        let mut ids: Vec<_> = session
            .state()
            .inventory_iter()
            .filter_map(|(&id, &quantity)| {
                if quantity <= 0 {
                    return None;
                }
                let kind = if tab == 1 {
                    self.catalog.properties.get(&id).map(|item| item.category)
                } else if tab == 2 {
                    self.catalog.equipment.get(&id).map(|item| match item.kind {
                        EquipmentType::Sword
                        | EquipmentType::DualSwords
                        | EquipmentType::Zither => 0,
                        EquipmentType::Head => 1,
                        EquipmentType::Body => 2,
                        EquipmentType::Feet => 3,
                        EquipmentType::Accessory => 4,
                    })
                } else {
                    None
                };
                kind.filter(|&kind| category == -1 || category == kind)
                    .map(|_| id)
            })
            .collect();
        ids.sort_unstable();
        ids
    }
}

impl IPal4MenuStateImpl for Pal4MenuState {
    fn preview_texture(&self, id: i32, width: f32, height: f32, yaw: f32) -> i32 {
        let tab = self.active_tab();
        if !(1..=2).contains(&tab) || !self.action_allowed(tab) {
            return 0;
        }
        self.preview
            .borrow_mut()
            .as_mut()
            .map_or(0, |preview| preview.request(tab, id, width, height, yaw))
    }
    fn leader(&self) -> i32 {
        self.session.borrow().state().leader() as i32
    }
    fn available(&self) -> bool {
        self.available.get()
    }
    fn active_tab(&self) -> i32 {
        self.session.borrow().menu_tab().unwrap_or(-1)
    }
    fn tab_enabled(&self, tab: i32) -> bool {
        (0..=2).contains(&tab) && self.session.borrow().state().menu_enabled(tab)
    }
    fn open(&self, tab: i32) -> bool {
        let session = self.session.borrow();
        if !self.available.get() && session.menu_tab().is_none() {
            return false;
        }
        session.request_menu(tab, session.menu_forced())
    }
    fn close(&self) {
        self.session.borrow().close_menu();
    }
    fn member_present(&self, role: i32) -> bool {
        self.session
            .borrow()
            .state()
            .player(role as usize)
            .is_some_and(|player| player.in_team)
    }
    fn member_name(&self, role: i32) -> &str {
        self.catalog
            .roles
            .get(&(role as usize))
            .map_or("", |role| role.name.as_str())
    }
    fn member_value(&self, role: i32, field: i32) -> i32 {
        let session = self.session.borrow();
        let Some(player) = session.state().player(role as usize) else {
            return 0;
        };
        match field {
            0 => player.level,
            1 => player.hp,
            2 => player.max_hp,
            3 => player.mp,
            4 => player.max_mp,
            5 => player.effective_stats.integer("CA_PHYSICAL"),
            6 => player.effective_stats.integer("CA_TOUGHNESS"),
            7 => player.effective_stats.integer("CA_SPEED"),
            8 => player.effective_stats.integer("CA_LUCKY"),
            9 => player.effective_stats.integer("CA_WILL"),
            10 => player.experience,
            11..=15 => player.effective_stats.integer(
                [
                    "EFE_WATER",
                    "EFE_FIRE",
                    "EFE_THUNDER",
                    "EFE_AIR",
                    "EFE_EARTH",
                ][(field - 11) as usize],
            ),
            _ => 0,
        }
    }
    fn money(&self) -> i32 {
        self.session.borrow().state().money()
    }
    fn play_seconds(&self, current_session: bool) -> i32 {
        self.session.borrow().play_seconds(current_session)
    }
    fn inventory_count(&self, tab: i32, category: i32) -> i32 {
        self.inventory_ids(tab, category).len() as i32
    }
    fn inventory_item(&self, tab: i32, category: i32, index: i32) -> i32 {
        self.inventory_ids(tab, category)
            .get(index as usize)
            .copied()
            .unwrap_or(0)
    }
    fn item_quantity(&self, id: i32) -> i32 {
        self.session.borrow().state().equipment_count(id)
    }
    fn item_name(&self, id: i32) -> &str {
        self.catalog
            .properties
            .get(&id)
            .map(|item| item.name.as_str())
            .or_else(|| {
                self.catalog
                    .equipment
                    .get(&id)
                    .map(|item| item.name.as_str())
            })
            .unwrap_or("")
    }
    fn item_description(&self, id: i32) -> &str {
        self.descriptions.get(&id).map_or("", String::as_str)
    }
    fn item_effect(&self, id: i32) -> &str {
        self.effects.get(&id).map_or("", String::as_str)
    }
    fn item_price(&self, id: i32) -> i32 {
        self.catalog
            .properties
            .get(&id)
            .map(|item| item.price)
            .or_else(|| self.catalog.equipment.get(&id).map(|item| item.price))
            .unwrap_or(0)
    }
    fn equipped_item(&self, role: i32, position: i32) -> i32 {
        let session = self.session.borrow();
        let Some(player) = session.state().player(role as usize) else {
            return 0;
        };
        let e = &player.equipment;
        match position {
            0 => e.weapon,
            1 => e.head,
            2 => e.body,
            3 => e.feet,
            4 => e.accessories[0],
            5 => e.accessories[1],
            _ => None,
        }
        .unwrap_or(0)
    }
    fn can_equip(&self, role: i32, id: i32) -> bool {
        if !self.action_allowed(2) || !self.member_present(role) {
            return false;
        }
        let session = self.session.borrow();
        self.catalog.can_equip(
            role as usize,
            session.state().player_level(role as usize),
            id,
        ) && session.state().has_equipment(id)
    }
    fn equip(&self, role: i32, id: i32) -> bool {
        self.can_equip(role, id)
            && self
                .session
                .borrow_mut()
                .state_mut()
                .equip_from_inventory(role as usize, id, &self.catalog)
                .is_ok()
    }
    fn can_use(&self, role: i32, id: i32) -> bool {
        self.action_allowed(1)
            && self
                .session
                .borrow()
                .state()
                .can_use_property(role as usize, id, &self.catalog)
    }
    fn use_item(&self, role: i32, id: i32) -> bool {
        self.action_allowed(1)
            && self
                .session
                .borrow_mut()
                .state_mut()
                .use_property(role as usize, id, &self.catalog)
    }
}

fn wrap_description(text: &str) -> String {
    let mut result = String::new();
    let mut width = 0;
    for c in text.chars().filter(|&c| c != '$' && c != '\r') {
        if c == '\n' {
            width = 0;
        } else if width >= 24 {
            result.push('\n');
            width = 0;
        }
        result.push(c);
        width += 1;
    }
    result
}
