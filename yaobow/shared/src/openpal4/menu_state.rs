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
    slots: RefCell<Vec<(i32, String)>>,
    scratch: RefCell<String>,
    audio_mix: std::sync::Arc<crate::audio_mix::AudioMix>,
    persist_audio: Cell<bool>,
}

ComObject_Pal4MenuState!(super::Pal4MenuState);

impl Pal4MenuState {
    pub fn create(session: Rc<RefCell<Pal4Session>>) -> ComRc<IPal4MenuState> {
        Self::create_internal(
            session,
            std::sync::Arc::new(crate::audio_mix::AudioMix::default()),
            false,
        )
    }

    fn create_internal(
        session: Rc<RefCell<Pal4Session>>,
        audio_mix: std::sync::Arc<crate::audio_mix::AudioMix>,
        persist_audio: bool,
    ) -> ComRc<IPal4MenuState> {
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
            slots: RefCell::new(Vec::new()),
            scratch: RefCell::new(String::new()),
            audio_mix,
            persist_audio: Cell::new(persist_audio),
        })
    }

    pub(crate) fn create_with_audio(
        session: Rc<RefCell<Pal4Session>>,
        audio_mix: std::sync::Arc<crate::audio_mix::AudioMix>,
    ) -> ComRc<IPal4MenuState> {
        Self::create_internal(session, audio_mix, true)
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
        if category == -1 {
            return session
                .state()
                .recent_inventory_ids(tab == 2)
                .into_iter()
                .filter(|id| {
                    if tab == 1 {
                        self.catalog.properties.contains_key(id)
                    } else {
                        tab == 2 && self.catalog.equipment.contains_key(id)
                    }
                })
                .collect();
        }
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
    fn refresh_slots(&self) {
        use super::states::persistent_state::Pal4PersistentState;
        let app_name = self.session.borrow().app_name().to_owned();
        let slots = (1..=Pal4PersistentState::SLOT_COUNT)
            .map(|slot| {
                let path = Pal4PersistentState::slot_path(&app_name, slot);
                match Pal4PersistentState::load(&app_name, slot) {
                    Ok(state) if !state.scene_name().is_empty() => {
                        let level = state.player(state.leader()).map_or(0, |p| p.level);
                        let time = state.play_seconds() as i32;
                        (
                            1,
                            format!(
                                "{slot:02}   {} / {}   Lv.{level}   {:02}:{:02}:{:02}",
                                state.scene_name(),
                                state.block_name(),
                                time / 3600,
                                time / 60 % 60,
                                time % 60
                            ),
                        )
                    }
                    Err(_) if !path.exists() => (0, format!("{slot:02}   空白存档")),
                    _ => (2, format!("{slot:02}   存档损坏或无法读取")),
                }
            })
            .collect();
        *self.slots.borrow_mut() = slots;
    }

    fn slot_state(&self, slot: i32) -> i32 {
        self.slots
            .borrow()
            .get(slot.wrapping_sub(1) as usize)
            .map_or(0, |s| s.0)
    }

    fn slot_summary(&self, slot: i32) -> &str {
        *self.scratch.borrow_mut() = self
            .slots
            .borrow()
            .get(slot.wrapping_sub(1) as usize)
            .map_or_else(String::new, |s| s.1.clone());
        // The COM adapter copies this string before the next accessor call.
        unsafe { (*self.scratch.as_ptr()).as_str() }
    }

    fn system_action(&self, action: i32, slot: i32) -> bool {
        use super::{session::SystemAction, states::persistent_state::Pal4PersistentState};
        if !self.available.get() || !self.action_allowed(6) {
            return false;
        }
        let valid_slot = (1..=Pal4PersistentState::SLOT_COUNT).contains(&slot);
        let action = match action {
            0 if valid_slot => SystemAction::Save(slot),
            1 if valid_slot && self.slot_state(slot) == 1 => SystemAction::Load(slot),
            2 => SystemAction::Title,
            3 => SystemAction::Exit,
            _ => return false,
        };
        self.session.borrow().request_system_action(action)
    }

    fn system_busy(&self) -> bool {
        self.session.borrow().system_busy()
    }

    fn system_message(&self) -> &str {
        *self.scratch.borrow_mut() = self.session.borrow().system_message().to_owned();
        unsafe { (*self.scratch.as_ptr()).as_str() }
    }

    fn audio_volume(&self, music: bool) -> f32 {
        if music {
            self.audio_mix.music()
        } else {
            self.audio_mix.sound()
        }
    }

    fn set_audio_volume(&self, music: bool, volume: f32) -> bool {
        if !self.action_allowed(6) {
            return false;
        }
        if music {
            self.audio_mix.set_music(volume)
        } else {
            self.audio_mix.set_sound(volume)
        }
    }

    fn save_audio_settings(&self) -> bool {
        if !self.persist_audio.get() || !self.action_allowed(6) {
            return false;
        }
        let mut config = crate::config::YaobowConfig::load();
        config.audio.music_volume = self.audio_mix.music();
        config.audio.sound_volume = self.audio_mix.sound();
        match config.save() {
            Ok(()) => true,
            Err(error) => {
                log::error!("Cannot save PAL4 audio settings: {error}");
                false
            }
        }
    }

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
        matches!(tab, 0..=2 | 6) && self.session.borrow().state().menu_enabled(tab)
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
            16 => player.rage,
            17 => super::states::persistent_state::PlayerState::MAX_RAGE,
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
