use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::GameType;

const CONFIG_FILE_NAME: &str = "yaobow.toml";
const ENV_OVERRIDE: &str = "YAOBOW_CONFIG";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct GameConfig {
    #[serde(default)]
    pub asset_path: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct YaobowConfig {
    #[serde(default)]
    pub game: BTreeMap<String, GameConfig>,
}

impl YaobowConfig {
    /// Returns the on-disk path the config is loaded from / saved to.
    /// Honors the `YAOBOW_CONFIG` env var; otherwise uses the OS config dir.
    pub fn config_path() -> PathBuf {
        if let Ok(v) = std::env::var(ENV_OVERRIDE) {
            if !v.is_empty() {
                return PathBuf::from(v);
            }
        }
        crate::ydirs::config_dir().join(CONFIG_FILE_NAME)
    }

    /// Loads the config. Never panics: missing or malformed files yield an
    /// empty config. A `./yaobow.toml` in the cwd is honored as a read-only
    /// dev fallback when the primary location does not exist.
    pub fn load() -> Self {
        let primary = Self::config_path();
        if let Some(cfg) = Self::try_load_from(&primary) {
            return cfg;
        }

        let cwd_fallback = PathBuf::from(CONFIG_FILE_NAME);
        if let Some(cfg) = Self::try_load_from(&cwd_fallback) {
            log::info!(
                "loaded yaobow config from cwd fallback: {}",
                cwd_fallback.display()
            );
            return cfg;
        }

        Self::default()
    }

    fn try_load_from(path: &std::path::Path) -> Option<Self> {
        if !path.exists() {
            return None;
        }
        match std::fs::read_to_string(path) {
            Ok(text) => match toml::from_str::<YaobowConfig>(&text) {
                Ok(cfg) => Some(cfg),
                Err(e) => {
                    log::warn!("failed to parse {}: {}", path.display(), e);
                    Some(Self::default())
                }
            },
            Err(e) => {
                log::warn!("failed to read {}: {}", path.display(), e);
                None
            }
        }
    }

    /// Persists the config to `config_path()`, creating parent dirs.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(&path, text)?;
        Ok(())
    }

    pub fn asset_path_for(&self, game: GameType) -> &str {
        self.game
            .get(game.config_key())
            .map(|g| g.asset_path.as_str())
            .unwrap_or("")
    }

    pub fn set_asset_path(&mut self, game: GameType, path: String) {
        self.game
            .entry(game.config_key().to_string())
            .or_insert_with(GameConfig::default)
            .asset_path = path;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env_override<T>(path: &std::path::Path, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var(ENV_OVERRIDE, path);
        let r = f();
        std::env::remove_var(ENV_OVERRIDE);
        r
    }

    #[test]
    fn load_missing_returns_default() {
        let dir = std::env::temp_dir().join(format!("yaobow-cfg-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("missing.toml");
        with_env_override(&path, || {
            let cfg = YaobowConfig::load();
            assert!(cfg.game.is_empty());
        });
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("yaobow-cfg-test-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("yaobow.toml");
        with_env_override(&path, || {
            let mut cfg = YaobowConfig::default();
            cfg.set_asset_path(GameType::PAL3, "/games/pal3".to_string());
            cfg.set_asset_path(GameType::PAL4, "/games/pal4".to_string());
            cfg.save().unwrap();

            let loaded = YaobowConfig::load();
            assert_eq!(loaded.asset_path_for(GameType::PAL3), "/games/pal3");
            assert_eq!(loaded.asset_path_for(GameType::PAL4), "/games/pal4");
            assert_eq!(loaded.asset_path_for(GameType::PAL5), "");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_path_honors_env_override() {
        let p = std::env::temp_dir().join("explicit-yaobow.toml");
        with_env_override(&p, || {
            assert_eq!(YaobowConfig::config_path(), p);
        });
    }
}
