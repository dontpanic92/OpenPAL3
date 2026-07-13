//! Minimal, read-only lookup of the running system's `yaobow.toml`
//! `[game.pal3] asset_path`, used only to seed a default candidate
//! root for detection/selection (see [`crate::environment`]).
//!
//! Deliberately independent of `yaobow::shared::config::YaobowConfig`
//! (this crate must not depend on `shared`, which would drag in the
//! full engine/scripting graph): this reads the *same* on-disk
//! `yaobow.toml` schema (`[game.<config_key>] asset_path = "..."`)
//! with a tiny, read-only parse, mirroring the path-resolution rules
//! documented in this workspace's `docs/BUILD_INSTRUCTIONS.md` /
//! agent instructions:
//!
//! - `YAOBOW_CONFIG` env var overrides the path outright.
//! - Otherwise: `~/Library/Application Support/yaobow/yaobow.toml`
//!   (macOS), `%APPDATA%\yaobow\yaobow.toml` (Windows),
//!   `~/.config/yaobow/yaobow.toml` (Linux).

use std::path::PathBuf;

use serde::Deserialize;

const ENV_OVERRIDE: &str = "YAOBOW_CONFIG";
const CONFIG_FILE_NAME: &str = "yaobow.toml";

#[derive(Deserialize, Default)]
struct GameConfig {
    #[serde(default)]
    asset_path: String,
}

#[derive(Deserialize, Default)]
struct YaobowConfigShape {
    #[serde(default)]
    game: std::collections::BTreeMap<String, GameConfig>,
}

/// Path `yaobow.toml` would be loaded from on this platform, honoring
/// `YAOBOW_CONFIG`.
pub fn config_path() -> Option<PathBuf> {
    if let Ok(v) = std::env::var(ENV_OVERRIDE) {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    config_dir().map(|d| d.join(CONFIG_FILE_NAME))
}

#[cfg(target_os = "macos")]
fn config_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join("Library/Application Support/yaobow"))
}

#[cfg(target_os = "windows")]
fn config_dir() -> Option<PathBuf> {
    let appdata = std::env::var("APPDATA").ok()?;
    Some(PathBuf::from(appdata).join("yaobow"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn config_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("yaobow"));
        }
    }
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".config/yaobow"))
}

/// Best-effort read of `[game.pal3] asset_path` from `yaobow.toml`.
/// Returns `None` on any failure (missing file, malformed TOML, empty
/// value) — this is only ever used to seed a *suggested* default, so a
/// failure here should never be fatal to the caller.
pub fn configured_pal3_asset_path() -> Option<PathBuf> {
    let path = config_path()?;
    let text = std::fs::read_to_string(&path).ok()?;
    let cfg: YaobowConfigShape = toml::from_str(&text).ok()?;
    let asset_path = cfg.game.get("pal3")?.asset_path.trim();
    if asset_path.is_empty() {
        None
    } else {
        Some(PathBuf::from(asset_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // `YAOBOW_CONFIG` is process-global state; serialize the two tests
    // below so they can't interleave and observe each other's value
    // when `cargo test` runs them on different threads.
    static ENV_GUARD: Mutex<()> = Mutex::new(());

    #[test]
    fn missing_config_file_yields_none() {
        let _guard = ENV_GUARD.lock().unwrap();
        // SAFETY: serialized by `ENV_GUARD` above.
        unsafe {
            std::env::set_var(
                ENV_OVERRIDE,
                "/nonexistent/path/definitely-missing/yaobow.toml",
            );
        }
        assert_eq!(configured_pal3_asset_path(), None);
        unsafe {
            std::env::remove_var(ENV_OVERRIDE);
        }
    }

    #[test]
    fn reads_configured_pal3_asset_path() {
        let _guard = ENV_GUARD.lock().unwrap();
        let dir = crate::test_scratch::dir("config-pal3-path");
        let config_path = dir.join("yaobow.toml");
        std::fs::write(
            &config_path,
            "[game.pal3]\nasset_path = \"/opt/games/pal3\"\n",
        )
        .unwrap();

        // SAFETY: serialized by `ENV_GUARD` above.
        unsafe {
            std::env::set_var(ENV_OVERRIDE, &config_path);
        }
        assert_eq!(
            configured_pal3_asset_path(),
            Some(PathBuf::from("/opt/games/pal3"))
        );
        unsafe {
            std::env::remove_var(ENV_OVERRIDE);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
