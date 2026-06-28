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

/// Per-app UI preferences. Currently just the imgui theme name.
/// Empty string means "use the built-in default".
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UiConfig {
    #[serde(default)]
    pub theme: String,
}

/// Resolution at which the 3D scene is rasterized. `Native` (default)
/// renders straight into the swapchain image (= physical pixels, so
/// on HiDPI displays a 2× Retina pays ~4× the per-frame pixel cost).
/// `Logical` renders the scene into an offscreen target sized to the
/// window's logical extent and upscales it for presentation, keeping
/// imgui at native resolution so UI/text stays sharp.
///
/// The runtime engine reads this on startup. Toggling at runtime
/// requires the engine to rebuild its offscreen target; see the
/// `IConfigService` setter for the hot-reload contract.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SceneScaleMode {
    /// Render the scene at the physical swapchain extent (current
    /// behavior; preserves the sharpest 3D image at the cost of
    /// HiDPI fragment-shader work).
    #[default]
    Native,
    /// Render the scene at the window's logical extent, then upscale
    /// to the swapchain image. Trades some 3D sharpness for a large
    /// fragment-cost reduction on HiDPI displays.
    Logical,
}

impl SceneScaleMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SceneScaleMode::Native => "native",
            SceneScaleMode::Logical => "logical",
        }
    }

    /// Parses the snake_case forms persisted in TOML. Unknown values
    /// fall back to `Native` so a typo in the config file never breaks
    /// rendering.
    pub fn from_str(value: &str) -> Self {
        match value {
            "logical" => SceneScaleMode::Logical,
            _ => SceneScaleMode::Native,
        }
    }
}

/// Engine-wide rendering preferences. Stored under `[render]` in
/// `yaobow.toml`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RenderConfig {
    /// See [`SceneScaleMode`]. Defaults to `Native` to preserve the
    /// historical behavior for existing installs.
    #[serde(default)]
    pub scene_scale_mode: SceneScaleMode,
}

/// Default master volume (linear gain). `0.7` (~-3 dB) gives audible
/// headroom so the game isn't louder than other apps at the same OS
/// volume. A free fn is required because serde's numeric default is
/// `0.0`.
fn default_master_volume() -> f32 {
    0.7
}

/// Audio preferences shared by the game runtime and editor. Stored
/// under `[audio]` in `yaobow.toml`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AudioConfig {
    /// Linear master volume in `[0.0, 1.0]` applied as the OpenAL
    /// listener gain. Scales BGM, SFX, voice, and video audio
    /// uniformly. Defaults to [`default_master_volume`].
    #[serde(default = "default_master_volume")]
    pub master_volume: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            master_volume: default_master_volume(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct YaobowConfig {
    #[serde(default)]
    pub game: BTreeMap<String, GameConfig>,

    /// UI preferences for the `yaobow` game runtime.
    #[serde(default)]
    pub yaobow: UiConfig,

    /// UI preferences for the `yaobow_editor`.
    #[serde(default)]
    pub editor: UiConfig,

    /// Rendering preferences shared by both the game runtime and the
    /// editor previewer.
    #[serde(default)]
    pub render: RenderConfig,

    /// Audio preferences (master volume) shared by both the game
    /// runtime and the editor.
    #[serde(default)]
    pub audio: AudioConfig,
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

    /// Theme name for the given `config_key`. Recognised keys are `"yaobow"`
    /// and `"editor"`; any other key yields an empty string (callers should
    /// treat empty as "use the built-in default").
    pub fn theme_for(&self, config_key: &str) -> &str {
        match config_key {
            "yaobow" => &self.yaobow.theme,
            "editor" => &self.editor.theme,
            _ => "",
        }
    }

    /// Persist a theme choice under `config_key`. Unknown keys are ignored.
    pub fn set_theme(&mut self, config_key: &str, theme: String) {
        match config_key {
            "yaobow" => self.yaobow.theme = theme,
            "editor" => self.editor.theme = theme,
            _ => log::warn!("ignoring set_theme for unknown config_key '{}'", config_key),
        }
    }

    /// Current scene-render scale mode. See [`SceneScaleMode`].
    pub fn scene_scale_mode(&self) -> SceneScaleMode {
        self.render.scene_scale_mode
    }

    /// Master audio volume as a linear gain, clamped to `[0.0, 1.0]`.
    /// Applied as the OpenAL listener gain at startup. A non-finite or
    /// out-of-range persisted value is sanitised here so the engine
    /// never receives an undefined gain.
    pub fn master_volume(&self) -> f32 {
        let v = self.audio.master_volume;
        if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            default_master_volume()
        }
    }

    /// Persist a new scene-render scale mode. Callers are responsible
    /// for triggering any engine-side recreate (typically through
    /// `IConfigService::save` + a `restart-required` UX, or a future
    /// engine hot-reload hook).
    pub fn set_scene_scale_mode(&mut self, mode: SceneScaleMode) {
        self.render.scene_scale_mode = mode;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env_override<T>(path: &std::path::Path, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var(ENV_OVERRIDE, path) };
        let r = f();
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(ENV_OVERRIDE) };
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

    #[test]
    fn theme_roundtrip() {
        let dir = std::env::temp_dir().join(format!("yaobow-cfg-test-th-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("yaobow.toml");
        with_env_override(&path, || {
            let mut cfg = YaobowConfig::default();
            cfg.set_theme("editor", "blender_dark".to_string());
            cfg.set_theme("yaobow", "yaobow".to_string());
            cfg.set_theme("unknown", "ignored".to_string());
            cfg.save().unwrap();

            let loaded = YaobowConfig::load();
            assert_eq!(loaded.theme_for("editor"), "blender_dark");
            assert_eq!(loaded.theme_for("yaobow"), "yaobow");
            assert_eq!(loaded.theme_for("unknown"), "");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scene_scale_mode_default_is_native() {
        let cfg = YaobowConfig::default();
        assert_eq!(cfg.scene_scale_mode(), SceneScaleMode::Native);
    }

    #[test]
    fn scene_scale_mode_roundtrip() {
        let dir = std::env::temp_dir().join(format!("yaobow-cfg-test-ss-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("yaobow.toml");
        with_env_override(&path, || {
            let mut cfg = YaobowConfig::default();
            cfg.set_scene_scale_mode(SceneScaleMode::Logical);
            cfg.save().unwrap();

            let loaded = YaobowConfig::load();
            assert_eq!(loaded.scene_scale_mode(), SceneScaleMode::Logical);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scene_scale_mode_from_str_falls_back_to_native() {
        assert_eq!(SceneScaleMode::from_str("native"), SceneScaleMode::Native);
        assert_eq!(SceneScaleMode::from_str("logical"), SceneScaleMode::Logical);
        assert_eq!(SceneScaleMode::from_str("garbage"), SceneScaleMode::Native);
        assert_eq!(SceneScaleMode::from_str(""), SceneScaleMode::Native);
    }

    #[test]
    fn scene_scale_mode_unknown_value_in_toml_falls_back_to_native() {
        // Defensive: a corrupt or hand-edited TOML must not break
        // rendering. serde would normally reject the unknown variant
        // and zero the whole render section back to its default,
        // which still yields `Native`. This pins that behavior.
        let dir =
            std::env::temp_dir().join(format!("yaobow-cfg-test-ssbad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("yaobow.toml");
        std::fs::write(
            &path,
            "[render]\nscene_scale_mode = \"definitely_not_a_mode\"\n",
        )
        .unwrap();
        with_env_override(&path, || {
            let loaded = YaobowConfig::load();
            assert_eq!(loaded.scene_scale_mode(), SceneScaleMode::Native);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn master_volume_default_is_headroom() {
        let cfg = YaobowConfig::default();
        assert_eq!(cfg.master_volume(), 0.7);
        assert_eq!(AudioConfig::default().master_volume, 0.7);
    }

    #[test]
    fn master_volume_serde_default_when_audio_absent() {
        // A config with no [audio] table must deserialize to the
        // default headroom volume, not serde's numeric `0.0`.
        let dir =
            std::env::temp_dir().join(format!("yaobow-cfg-test-volabs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("yaobow.toml");
        std::fs::write(&path, "[render]\nscene_scale_mode = \"native\"\n").unwrap();
        with_env_override(&path, || {
            let loaded = YaobowConfig::load();
            assert_eq!(loaded.master_volume(), 0.7);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn master_volume_explicit_roundtrips() {
        let dir =
            std::env::temp_dir().join(format!("yaobow-cfg-test-volrt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("yaobow.toml");
        std::fs::write(&path, "[audio]\nmaster_volume = 0.4\n").unwrap();
        with_env_override(&path, || {
            let loaded = YaobowConfig::load();
            assert_eq!(loaded.master_volume(), 0.4);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn master_volume_clamps_out_of_range() {
        let mut cfg = YaobowConfig::default();
        cfg.audio.master_volume = 2.0;
        assert_eq!(cfg.master_volume(), 1.0);
        cfg.audio.master_volume = -1.0;
        assert_eq!(cfg.master_volume(), 0.0);
        cfg.audio.master_volume = f32::NAN;
        assert_eq!(cfg.master_volume(), 0.7);
    }
}
