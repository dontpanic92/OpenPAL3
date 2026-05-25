//! Canonical `IConfigService` exposed to p7 scripts.
//!
//! Both `yaobow` and `yaobow_editor` use this one impl — the IDL class
//! is declared in `crosscom/idl/shared_services.idl`, so the
//! `ComObject_ConfigService!` macro is generated in `shared` and the
//! orphan rule is satisfied here.
//!
//! The interface is keyed by `&str config_key` (matching
//! `GameType::config_key()`); ordinal axes belong to `IGameRegistry`,
//! not to the config surface.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;

use radiance_scripting::comdef::services::{IConfigService, IConfigServiceImpl};

use crate::config::YaobowConfig;
use crate::GameType;

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
}

impl IConfigServiceImpl for ConfigService {
    fn get_asset_path(&self, config_key: &str) -> &str {
        let value = match GameType::from_config_key(config_key) {
            Some(g) => self.config.borrow().asset_path_for(g).to_string(),
            None => String::new(),
        };
        *self.last_string.borrow_mut() = value;
        // SAFETY: the returned slice points at `last_string`. The
        // dispatcher copies it into a CString synchronously before any
        // subsequent call can mutate `last_string`, and the runtime is
        // single-threaded.
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }

    fn set_asset_path(&self, config_key: &str, path: &str) {
        if let Some(g) = GameType::from_config_key(config_key) {
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
