use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
};

use crosscom::ComRc;
use fileformats::pal3::scene_names::SceneNames;

use super::audio_mix::Pal3AudioMix;
use super::{
    comdef::{IPal3MenuState, IPal3MenuStateImpl},
    loaders::stats::StatsData,
    states::persistent_state::PersistentState,
};

pub(crate) const EARTH_PEARL_PROC: u32 = 2101;

pub struct Pal3MenuState {
    state: Rc<RefCell<PersistentState>>,
    data: Option<Rc<StatsData>>,
    scene_names: Rc<SceneNames>,
    descriptions: HashMap<u32, String>,
    pending_load: Cell<Option<i32>>,
    pending_return_to_menu: Cell<bool>,
    earth_pearl_available: Cell<bool>,
    pending_earth_pearl: Cell<bool>,
    load_failed: Cell<bool>,
    audio_mix: Arc<Pal3AudioMix>,
    audio_persistence: AudioPersistence,
    preview: RefCell<Option<super::menu_preview::MenuPreview>>,
    effects: RefCell<super::menu_effects::ItemEffects>,
    sounds: RefCell<Vec<&'static str>>,
    menu_was_open: Cell<bool>,
    thumbnail_uploader: RefCell<Option<radiance_scripting::services::sprite::SpriteUploader>>,
}

enum AudioPersistence {
    Disabled,
    UserConfig,
    Path(PathBuf),
}

ComObject_Pal3MenuState!(super::Pal3MenuState);

impl Pal3MenuState {
    pub fn create(
        state: Rc<RefCell<PersistentState>>,
        data: Option<Rc<StatsData>>,
    ) -> ComRc<IPal3MenuState> {
        Self::create_internal(
            state,
            data,
            Rc::default(),
            Arc::new(Pal3AudioMix::default()),
            AudioPersistence::Disabled,
        )
    }

    pub fn create_with_audio(
        state: Rc<RefCell<PersistentState>>,
        data: Option<Rc<StatsData>>,
        audio_mix: Arc<Pal3AudioMix>,
    ) -> ComRc<IPal3MenuState> {
        Self::create_internal(
            state,
            data,
            Rc::default(),
            audio_mix,
            AudioPersistence::Disabled,
        )
    }

    pub fn create_with_audio_config_path(
        state: Rc<RefCell<PersistentState>>,
        data: Option<Rc<StatsData>>,
        audio_mix: Arc<Pal3AudioMix>,
        config_path: PathBuf,
    ) -> ComRc<IPal3MenuState> {
        Self::create_internal(
            state,
            data,
            Rc::default(),
            audio_mix,
            AudioPersistence::Path(config_path),
        )
    }

    pub(crate) fn create_with_persisted_audio(
        state: Rc<RefCell<PersistentState>>,
        data: Option<Rc<StatsData>>,
        audio_mix: Arc<Pal3AudioMix>,
        scene_names: Rc<SceneNames>,
    ) -> ComRc<IPal3MenuState> {
        Self::create_internal(
            state,
            data,
            scene_names,
            audio_mix,
            AudioPersistence::UserConfig,
        )
    }

    fn create_internal(
        state: Rc<RefCell<PersistentState>>,
        data: Option<Rc<StatsData>>,
        scene_names: Rc<SceneNames>,
        audio_mix: Arc<Pal3AudioMix>,
        audio_persistence: AudioPersistence,
    ) -> ComRc<IPal3MenuState> {
        state.borrow_mut().set_thumbnail(None);
        let descriptions = data
            .iter()
            .flat_map(|data| data.items())
            .map(|item| (item.id, clean_description(&item.description)))
            .collect();
        ComRc::from_object(Self {
            state,
            data,
            scene_names,
            descriptions,
            pending_load: Cell::new(None),
            pending_return_to_menu: Cell::new(false),
            earth_pearl_available: Cell::new(false),
            pending_earth_pearl: Cell::new(false),
            load_failed: Cell::new(false),
            audio_mix,
            audio_persistence,
            preview: RefCell::new(None),
            effects: RefCell::new(super::menu_effects::ItemEffects::default()),
            sounds: RefCell::new(Vec::new()),
            menu_was_open: Cell::new(false),
            thumbnail_uploader: RefCell::new(None),
        })
    }

    pub fn take_requested_load(&self) -> Option<i32> {
        self.pending_load.take()
    }

    pub(crate) fn set_earth_pearl_context(
        &self,
        scene_type: u32,
        scene_name: &str,
        has_return_proc: bool,
    ) {
        // Native 0x49ce82 excludes M22; 0x49d314 requires procedure 2101.
        self.earth_pearl_available
            .set(scene_type == 2 && !scene_name.eq_ignore_ascii_case("M22") && has_return_proc);
    }

    pub(crate) fn take_earth_pearl_request(&self) -> bool {
        self.pending_earth_pearl.take()
            && self.earth_pearl_available.get()
            && self.state.borrow().inventory().contains(6427)
    }

    pub(crate) fn capture_save_thumbnail(
        &self,
        engine: &mut dyn radiance::rendering::RenderingEngine,
    ) {
        if !self.menu_was_open.get() {
            let png = engine
                .capture_last_frame()
                .and_then(super::save_slots::encode_thumbnail);
            self.state.borrow_mut().set_thumbnail(png);
        }
    }

    pub fn take_requested_return_to_menu(&self) -> bool {
        self.pending_return_to_menu.replace(false)
    }

    pub fn report_load_failed(&self) {
        self.load_failed.set(true);
    }

    pub(crate) fn install_preview(&self, preview: super::menu_preview::MenuPreview) {
        *self.preview.borrow_mut() = Some(preview);
    }

    pub(crate) fn install_thumbnail_uploader(
        &self,
        uploader: radiance_scripting::services::sprite::SpriteUploader,
    ) {
        *self.thumbnail_uploader.borrow_mut() = Some(uploader);
    }

    pub(crate) fn render_preview(
        &self,
        open: bool,
        engine: &mut dyn radiance::rendering::RenderingEngine,
    ) {
        if open != self.menu_was_open.replace(open) || !open {
            let png = if open {
                engine
                    .capture_last_frame()
                    .and_then(super::save_slots::encode_thumbnail)
            } else {
                None
            };
            self.state.borrow_mut().set_thumbnail(png);
        }
        if !open {
            self.effects.borrow_mut().clear();
        }
        if let Some(preview) = self.preview.borrow_mut().as_mut() {
            preview.render(open, engine);
        }
    }

    pub fn take_menu_sounds(&self) -> Vec<&'static str> {
        std::mem::take(&mut *self.sounds.borrow_mut())
    }

    fn save_audio_settings(&self) -> bool {
        let (mut config, path) = match &self.audio_persistence {
            AudioPersistence::Disabled => return false,
            AudioPersistence::UserConfig => (crate::config::YaobowConfig::load(), None),
            AudioPersistence::Path(path) => (
                crate::config::YaobowConfig::load_from_path(path),
                Some(path.as_path()),
            ),
        };
        config.audio.music_volume = self.audio_mix.music();
        config.audio.sound_volume = self.audio_mix.sound();
        let result = match path {
            Some(path) => config.save_to_path(path),
            None => config.save(),
        };
        match result {
            Ok(()) => true,
            Err(error) => {
                log::error!("Cannot persist PAL3 audio volume: {error}");
                false
            }
        }
    }
}

impl IPal3MenuStateImpl for Pal3MenuState {
    fn native_page_preview_texture(&self, page: i32) -> i32 {
        self.preview
            .borrow()
            .as_ref()
            .map_or(0, |preview| preview.request_page(page))
    }

    fn item_preview_texture(&self, id: i32, equipment: bool) -> i32 {
        self.preview
            .borrow()
            .as_ref()
            .map_or(0, |preview| preview.request(id, equipment))
    }

    fn request_return_to_menu(&self) -> bool {
        if !self.available() {
            return false;
        }
        self.pending_load.set(None);
        self.pending_return_to_menu.set(true);
        true
    }

    fn can_equip_item(&self, role: i32, slot: i32, id: i32) -> bool {
        self.data
            .as_ref()
            .is_some_and(|data| self.state.borrow().can_equip_item(role, slot, id, data))
    }

    fn equip_item(&self, role: i32, slot: i32, id: i32) -> bool {
        self.data
            .as_ref()
            .is_some_and(|data| self.state.borrow_mut().equip_item(role, slot, id, data))
    }

    fn item_use_scope(&self, id: i32) -> i32 {
        if id == 6427 {
            return self.data.as_ref().map_or(0, |data| {
                data.item(6427).map_or(0, |item| {
                    if item.category == 3
                        && item.use_kind == 3
                        && item.use_context & 2 != 0
                        && item.use_scope == 2
                    {
                        2
                    } else {
                        0
                    }
                })
            });
        }
        self.data
            .as_ref()
            .map_or(0, |data| self.state.borrow().item_use_scope(id, data))
    }

    fn can_use_item(&self, role: i32, id: i32) -> bool {
        if id == 6427 {
            return role == -1
                && self.item_use_scope(id) == 2
                && self.earth_pearl_available.get()
                && !self.pending_earth_pearl.get()
                && self.state.borrow().inventory().contains(6427);
        }
        self.data
            .as_ref()
            .is_some_and(|data| self.state.borrow().can_use_item(role, id, data))
    }

    fn use_item(&self, role: i32, id: i32) -> bool {
        if id == 6427 {
            if !self.can_use_item(role, id) {
                return false;
            }
            // Dispatch after p7 returns. Native 0x49e5fd never consumes this item.
            self.pending_earth_pearl.set(true);
            return true;
        }
        let Some(data) = self.data.as_ref() else {
            return false;
        };
        let mut state = self.state.borrow_mut();
        let roles: Vec<_> = state
            .party()
            .members()
            .filter(|&member| {
                (role == -1 || member == role)
                    && state.can_use_item(if role == -1 { -1 } else { member }, id, data)
            })
            .collect();
        if !state.use_item(role, id, data) {
            return false;
        }
        if let Some(item) = data.item(id as u32) {
            for role in roles {
                if item.use_scope != 2 || state.stats().item_applies(role, item, data) {
                    self.effects.borrow_mut().spawn(role as usize, item);
                }
            }
        }
        true
    }

    fn play_item_menu_sound(&self, accepted: bool) {
        self.sounds
            .borrow_mut()
            .push(if accepted { "wa002" } else { "wa004" });
    }

    fn paint_item_effects(
        &self,
        ui: ComRc<radiance::comdef::IUiHost>,
        atlas: ComRc<super::comdef::IPal3UiAtlas>,
        role: i32,
        x: f32,
        y: f32,
        scale: f32,
    ) {
        self.effects
            .borrow_mut()
            .paint(ui, atlas, role as usize, x, y, scale);
    }

    fn item_target_hint(&self, id: i32) -> i32 {
        // Supported resource/poison subset; full native 0x49c660 scoring remains unverified.
        let Some(item) = self.data.as_ref().and_then(|data| data.item(id as u32)) else {
            return -1;
        };
        let state = self.state.borrow();
        let mut best = (-1, -1i32);
        for role in state.party().members() {
            if !self.can_use_item(if item.use_scope == 2 { -1 } else { role }, id) {
                continue;
            }
            let current = state.stats().character(role).unwrap().current();
            let maximum = state
                .stats()
                .attributes(role, self.data.as_ref().unwrap())
                .unwrap();
            let mut score = 0i32;
            for (i, deficit) in [
                maximum.hp - current.hp,
                maximum.qi - current.qi,
                maximum.mp - current.mp,
            ]
            .into_iter()
            .enumerate()
            {
                if item.amounts[i] > 0 {
                    score = score.wrapping_add(deficit);
                }
            }
            for (i, &poison) in state
                .stats()
                .character(role)
                .unwrap()
                .poisons()
                .iter()
                .enumerate()
            {
                if item.conditions[i] != 0 {
                    score = score.wrapping_add(poison as i32);
                }
            }
            if score > best.1 {
                best = (role, score);
            }
        }
        best.0
    }

    fn music_volume(&self) -> f32 {
        self.audio_mix.music()
    }

    fn sound_volume(&self) -> f32 {
        self.audio_mix.sound()
    }

    fn set_music_volume(&self, volume: f32) -> bool {
        self.audio_mix.set_music(volume)
    }

    fn set_sound_volume(&self, volume: f32) -> bool {
        self.audio_mix.set_sound(volume)
    }

    fn save_audio_settings(&self) -> bool {
        Pal3MenuState::save_audio_settings(self)
    }

    fn save_slots(&self) -> ComRc<super::comdef::IPal3SaveSlots> {
        super::save_slots::Pal3SaveSlots::snapshot_with_thumbnails(
            self.state.borrow().app_name(),
            self.thumbnail_uploader.borrow().as_ref(),
            Some(&self.scene_names),
        )
    }

    fn save_slot(&self, slot: i32) -> bool {
        if !self.available() || !(1..=super::save_slots::SAVE_SLOT_COUNT).contains(&slot) {
            return false;
        }
        let state = self.state.borrow();
        if state.scene_name().is_none() || state.sub_scene_name().is_none() {
            return false;
        }
        match state.try_save(slot) {
            Ok(()) => true,
            Err(error) => {
                log::error!("Cannot save slot {slot}: {error}");
                false
            }
        }
    }

    fn request_load(&self, slot: i32) -> bool {
        if !self.available() || !(1..=super::save_slots::SAVE_SLOT_COUNT).contains(&slot) {
            return false;
        }
        if PersistentState::try_load(self.state.borrow().app_name(), slot).is_err() {
            return false;
        }
        self.pending_load.set(Some(slot));
        self.load_failed.set(false);
        true
    }

    fn take_load_failed(&self) -> bool {
        self.load_failed.replace(false)
    }

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

    fn item_category(&self, id: i32) -> i32 {
        self.data
            .as_ref()
            .and_then(|data| data.item(id as u32))
            .map_or(0, |item| item.category as i32)
    }
}

fn clean_description(text: &str) -> String {
    text.chars().filter(|&ch| ch != '$' && ch != '\r').collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openpal3::{
        loaders::stats::{test_data, test_data_with_items},
        states::stats::Resource,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_PATH_ID: AtomicUsize = AtomicUsize::new(0);

    fn isolated_config_path(name: &str) -> PathBuf {
        let id = TEST_PATH_ID.fetch_add(1, Ordering::Relaxed);
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-data")
            .join(format!("pal3-audio-{name}-{}-{id}", std::process::id()))
            .join("yaobow.toml")
    }

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
        assert!(!ui.can_equip_item(0, 5, 82201));
        assert!(!ui.equip_item(0, 5, 82201));
        assert_eq!(ui.item_use_scope(6201), 0);
        assert!(!ui.can_use_item(0, 6201));
        assert!(!ui.use_item(0, 6201));
        assert_eq!(ui.music_volume(), 1.0);
        assert_eq!(ui.sound_volume(), 1.0);
        assert!(ui.set_music_volume(0.25));
        assert!(ui.set_sound_volume(2.0));
        assert_eq!(ui.music_volume(), 0.25);
        assert_eq!(ui.sound_volume(), 1.0);
        assert!(!ui.set_music_volume(f32::NAN));
        assert_eq!(ui.music_volume(), 0.25);
        assert!(!ui.save_audio_settings());
    }

    fn earth_pearl_data() -> Rc<StatsData> {
        Rc::new(test_data_with_items(&[
            fileformats::pal3::gdb::ItemDefinition {
                id: 6427,
                category: 3,
                use_kind: 3,
                use_scope: 2,
                use_context: 2,
                ..Default::default()
            },
        ]))
    }

    #[test]
    fn earth_pearl_requires_native_maze_context_and_queues_a_reusable_action() {
        let data = earth_pearl_data();
        let state = Rc::new(RefCell::new(PersistentState::new("test".into())));
        let ui = Pal3MenuState::create(state.clone(), Some(data.clone()));
        let menu = ui.inner::<Pal3MenuState>();
        menu.set_earth_pearl_context(2, "m01", true);
        assert_eq!(ui.item_use_scope(6427), 2);
        assert!(!ui.use_item(-1, 6427));
        state.borrow_mut().grant_world_item(6427, &data);

        for (kind, scene, has_proc) in [
            (0, "q01", true),
            (1, "m02", true),
            (2, "M22", true),
            (2, "m22", true),
            (2, "m01", false),
        ] {
            menu.set_earth_pearl_context(kind, scene, has_proc);
            assert!(!ui.can_use_item(-1, 6427));
            assert!(!ui.use_item(-1, 6427));
            assert!(!menu.take_earth_pearl_request());
            assert_eq!(state.borrow().inventory().quantity(6427), 1);
        }

        menu.set_earth_pearl_context(2, "m01", true);
        assert!(!ui.use_item(0, 6427));
        for _ in 0..2 {
            assert!(ui.can_use_item(-1, 6427));
            assert!(ui.use_item(-1, 6427));
            assert!(!ui.use_item(-1, 6427), "one outstanding request per frame");
            assert_eq!(state.borrow().inventory().quantity(6427), 1);
            assert!(menu.take_earth_pearl_request());
            assert!(!menu.take_earth_pearl_request());
            assert_eq!(state.borrow().inventory().quantity(6427), 1);
        }
    }

    #[test]
    fn earth_pearl_drops_stale_requests_without_touching_the_bag_or_links() {
        let data = earth_pearl_data();
        let state = Rc::new(RefCell::new(PersistentState::new("test".into())));
        let ui = Pal3MenuState::create(state.clone(), Some(data.clone()));
        let menu = ui.inner::<Pal3MenuState>();
        state.borrow_mut().add_item(6123, 1, &data);
        let before = state.borrow().inventory().clone();
        menu.set_earth_pearl_context(2, "m01", true);
        assert!(ui.use_item(-1, 6427));
        menu.set_earth_pearl_context(2, "m22", true);
        assert!(!menu.take_earth_pearl_request());
        assert_eq!(*state.borrow().inventory(), before);
        menu.set_earth_pearl_context(2, "m01", true);
        assert!(
            !menu.take_earth_pearl_request(),
            "stale requests do not retry"
        );
        assert!(ui.use_item(-1, 6427));
        state.borrow_mut().remove_item(6427, &data);
        let after_removal = state.borrow().inventory().clone();
        assert!(!menu.take_earth_pearl_request());
        assert_eq!(*state.borrow().inventory(), after_removal);
    }

    #[test]
    fn injected_audio_mix_is_live_and_never_persists_user_config() {
        let mix = Arc::new(Pal3AudioMix::new(0.2, 0.8));
        let ui = Pal3MenuState::create_with_audio(
            Rc::new(RefCell::new(PersistentState::new("test".into()))),
            None,
            mix.clone(),
        );
        assert_eq!(ui.music_volume(), 0.2);
        assert_eq!(ui.sound_volume(), 0.8);
        assert!(ui.set_music_volume(0.4));
        assert!(ui.set_sound_volume(0.6));
        assert_eq!(mix.music(), 0.4);
        assert_eq!(mix.sound(), 0.6);
        assert!(!ui.save_audio_settings());
    }

    #[test]
    fn isolated_audio_config_path_preserves_other_settings_and_both_channels() {
        let path = isolated_config_path("success");
        let root = path.parent().unwrap();
        let _ = std::fs::remove_dir_all(root);

        let mut config = crate::config::YaobowConfig::default();
        config.audio.master_volume = 0.45;
        config.set_asset_path(crate::GameType::PAL4, "pal4-assets".into());
        config.set_theme("yaobow", "yaobow".into());
        config.save_to_path(&path).unwrap();

        let mix = Arc::new(Pal3AudioMix::new(0.3, 0.7));
        let ui = Pal3MenuState::create_with_audio_config_path(
            Rc::new(RefCell::new(PersistentState::new("test".into()))),
            None,
            mix,
            path.clone(),
        );
        assert!(ui.save_audio_settings());

        let saved = crate::config::YaobowConfig::load_from_path(&path);
        assert_eq!(saved.music_volume(), 0.3);
        assert_eq!(saved.sound_volume(), 0.7);
        assert_eq!(saved.master_volume(), 0.45);
        assert_eq!(saved.asset_path_for(crate::GameType::PAL4), "pal4-assets");
        assert_eq!(saved.theme_for("yaobow"), "yaobow");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn isolated_audio_config_path_reports_write_failure() {
        let path = isolated_config_path("failure");
        let root = path.parent().unwrap();
        let _ = std::fs::remove_dir_all(root);
        std::fs::create_dir_all(root).unwrap();
        let blocker = root.join("not-a-directory");
        std::fs::write(&blocker, b"block config directory creation").unwrap();

        let ui = Pal3MenuState::create_with_audio_config_path(
            Rc::new(RefCell::new(PersistentState::new("test".into()))),
            None,
            Arc::new(Pal3AudioMix::new(0.3, 0.7)),
            blocker.join("yaobow.toml"),
        );
        assert!(!ui.save_audio_settings());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn description_is_plain_text_and_leaves_wrapping_to_the_menu() {
        assert_eq!(clean_description("$test\r\n$next"), "test\nnext");
        let text = format!("{}abc", "水".repeat(32));
        assert_eq!(clean_description(&text), text);
        assert_eq!(clean_description(""), "");
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
