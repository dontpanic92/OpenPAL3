//! Implementation of `IConfigService` exposed to p7 scripts.
//!
//! Wraps a shared `Rc<RefCell<YaobowConfig>>` so the welcome page, settings
//! page, and host can all observe and mutate the same in-memory state.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;

use radiance_scripting::comdef::services::{IConfigService, IConfigServiceImpl};
use shared::config::YaobowConfig;
use shared::GameType;

pub struct ConfigService {
    config: Rc<RefCell<YaobowConfig>>,
    last_string: RefCell<String>,
}

ComObject_ConfigService!(super::ConfigService);

impl ConfigService {
    pub fn create(config: Rc<RefCell<YaobowConfig>>) -> ComRc<IConfigService> {
        ComRc::from_object(Self {
            config,
            last_string: RefCell::new(String::new()),
        })
    }

    fn game_from_ordinal(ordinal: i32) -> Option<GameType> {
        match ordinal {
            0 => Some(GameType::PAL3),
            1 => Some(GameType::PAL3A),
            2 => Some(GameType::PAL4),
            3 => Some(GameType::PAL5),
            4 => Some(GameType::PAL5Q),
            5 => Some(GameType::SWD5),
            6 => Some(GameType::SWDHC),
            7 => Some(GameType::SWDCF),
            8 => Some(GameType::Gujian),
            9 => Some(GameType::Gujian2),
            _ => None,
        }
    }
}

impl IConfigServiceImpl for ConfigService {
    fn get_asset_path(&self, game: i32) -> &str {
        let value = match Self::game_from_ordinal(game) {
            Some(g) => self.config.borrow().asset_path_for(g).to_string(),
            None => String::new(),
        };
        *self.last_string.borrow_mut() = value;
        // SAFETY: read the String through RefCell::as_ptr to obtain a borrow
        // tied to &self rather than to the (already-dropped) RefMut guard.
        // The codegen copies the &str into a CString immediately on return,
        // and the dispatcher is single-threaded, so the slice cannot be
        // mutated before that copy completes.
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }

    fn set_asset_path(&self, game: i32, path: &str) {
        if let Some(g) = Self::game_from_ordinal(game) {
            self.config.borrow_mut().set_asset_path(g, path.to_string());
        }
    }

    fn save(&self) -> bool {
        match self.config.borrow().save() {
            Ok(()) => true,
            Err(e) => {
                log::warn!("failed to save yaobow config: {}", e);
                false
            }
        }
    }

    fn reload(&self) {
        *self.config.borrow_mut() = YaobowConfig::load();
    }

    fn pick_folder(&self, initial: &str) -> &str {
        let picked = pick_folder_native(initial);
        *self.last_string.borrow_mut() = picked;
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn pick_folder_native(initial: &str) -> String {
    use native_dialog::FileDialogBuilder;
    let mut dialog = FileDialogBuilder::default();
    if !initial.is_empty() {
        dialog = dialog.set_location(initial);
    }
    match dialog.open_single_dir().show() {
        Ok(Some(path)) => path.to_string_lossy().to_string(),
        Ok(None) => String::new(),
        Err(e) => {
            log::warn!("folder picker failed: {}", e);
            String::new()
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn pick_folder_native(_initial: &str) -> String {
    String::new()
}
