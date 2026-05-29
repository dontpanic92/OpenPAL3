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

use radiance::imgui::{available_themes, ImguiContext};
use radiance_scripting::comdef::services::{IConfigService, IConfigServiceImpl};

use crate::config::YaobowConfig;
use crate::GameType;

pub struct ConfigService {
    config: Rc<RefCell<YaobowConfig>>,
    /// Optional handle to the live imgui context so script-driven theme
    /// changes can be applied immediately. Headless tests pass `None`.
    imgui: Option<Rc<ImguiContext>>,
    last_string: RefCell<String>,
}

ComObject_ConfigService!(super::ConfigService);

impl ConfigService {
    pub fn create(config: Rc<RefCell<YaobowConfig>>) -> ComRc<IConfigService> {
        Self::create_with_imgui(config, None)
    }

    /// Variant that also wires the live `ImguiContext`, enabling
    /// `apply_theme` to push a theme to the running UI. Pass `None` from
    /// headless tests.
    pub fn create_with_imgui(
        config: Rc<RefCell<YaobowConfig>>,
        imgui: Option<Rc<ImguiContext>>,
    ) -> ComRc<IConfigService> {
        ComRc::from_object(Self {
            config,
            imgui,
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

    fn pick_save_file(&self, initial: &str, default_name: &str, ext_filter: &str) -> &str {
        let picked = pick_save_file_native(initial, default_name, ext_filter);
        *self.last_string.borrow_mut() = picked;
        // SAFETY: see ConfigService::get_asset_path.
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }

    fn get_theme(&self, config_key: &str) -> &str {
        let value = self.config.borrow().theme_for(config_key).to_string();
        *self.last_string.borrow_mut() = value;
        // SAFETY: see ConfigService::get_asset_path.
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }

    fn set_theme(&self, config_key: &str, name: &str) {
        self.config
            .borrow_mut()
            .set_theme(config_key, name.to_string());
    }

    fn apply_theme(&self, name: &str) {
        match &self.imgui {
            Some(ctx) => {
                ctx.apply_theme(name);
            }
            None => log::debug!(
                "apply_theme('{}') ignored: ConfigService has no ImguiContext",
                name
            ),
        }
    }

    fn available_theme_count(&self) -> i32 {
        available_themes().count() as i32
    }

    fn available_theme_at(&self, index: i32) -> &str {
        let value = if index < 0 {
            String::new()
        } else {
            available_themes()
                .nth(index as usize)
                .map(|s| s.to_string())
                .unwrap_or_default()
        };
        *self.last_string.borrow_mut() = value;
        // SAFETY: see ConfigService::get_asset_path.
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

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn pick_save_file_native(initial: &str, default_name: &str, ext_filter: &str) -> String {
    use native_dialog::FileDialogBuilder;
    let mut dialog = FileDialogBuilder::default();
    if !initial.is_empty() {
        dialog = dialog.set_location(initial);
    }
    if !default_name.is_empty() {
        dialog = dialog.set_filename(default_name);
    }
    if !ext_filter.is_empty() {
        // Single-extension filter matches the editor's only caller
        // today ("glb"); the description doubles as the dropdown
        // label and is harmless when it duplicates the extension.
        dialog = dialog.add_filter(ext_filter, &[ext_filter]);
    }
    let result = match dialog.save_single_file().show() {
        Ok(Some(path)) => path.to_string_lossy().to_string(),
        Ok(None) => String::new(),
        Err(e) => {
            log::warn!("save-file picker failed: {}", e);
            String::new()
        }
    };
    // Some platforms / desktop environments do not append the
    // selected extension automatically. Always normalize so callers
    // get a deterministic suffix.
    if !result.is_empty() && !ext_filter.is_empty() {
        let needs_ext = std::path::Path::new(&result)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| !e.eq_ignore_ascii_case(ext_filter))
            .unwrap_or(true);
        if needs_ext {
            return format!("{}.{}", result, ext_filter);
        }
    }
    result
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn pick_save_file_native(_initial: &str, _default_name: &str, _ext_filter: &str) -> String {
    String::new()
}
