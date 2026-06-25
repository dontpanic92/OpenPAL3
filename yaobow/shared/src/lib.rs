use loaders::{Pal4TextureResolver, dff::DffLoaderConfig};

use crate::loaders::{Pal5TextureResolver, Swd5TextureResolver};

pub mod agent_common;
pub mod config;
#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/shared_services_comdef.rs"));
}
pub mod config_service;
pub mod exporters;
pub mod loaders;
pub mod openpal3;
pub mod openpal4;
pub mod openpal5;
pub mod openswd5;
pub mod scripting;
pub mod theme_runtime;
/// Auto-generated script bridges from `[protosept(scriptable)]` IDLs.
pub mod script_bridges {
    pub mod pal4_debug {
        include!(concat!(env!("OUT_DIR"), "/shared_pal4_debug_bridge.rs"));
    }
    pub mod openpal4 {
        include!(concat!(env!("OUT_DIR"), "/shared_openpal4_bridge.rs"));
    }
    pub mod openpal3 {
        include!(concat!(env!("OUT_DIR"), "/shared_openpal3_bridge.rs"));
    }
    pub mod openpal5 {
        include!(concat!(env!("OUT_DIR"), "/shared_openpal5_bridge.rs"));
    }
    pub mod openswd5 {
        include!(concat!(env!("OUT_DIR"), "/shared_openswd5_bridge.rs"));
    }
    // Re-export the cross-IDL bridges that `openpal4` depends on so
    // the codegen-emitted `crate::script_bridges::radiance::...`
    // paths resolve. The actual bridge code lives in the
    // `radiance_scripting` crate; we just publish it at the same
    // local path the generator assumes.
    pub mod radiance {
        pub use radiance_scripting::script_bridges::radiance::*;
    }
}
pub mod ui;
pub mod utils;
pub mod video;
pub mod ydirs;

/// In-binary `.ypk` script bundle produced by `build.rs` from
/// `scripts/` + the codegen-derived IDL p7s. Mounted at `/shared/`
/// on the script `AssetManager` by [`mount_scripts`].
const SCRIPT_BUNDLE_YPK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shared_scripts.ypk"));

/// Mounts this crate's `shared_scripts.ypk` at `/shared/` on the
/// script `AssetManager`, so scripts can `import shared.openpal4;`,
/// `import shared.openpal3;`, `import shared.pal4_debug;`, etc.
pub fn mount_scripts(assets: &radiance::asset::AssetManager) {
    assets
        .mount_ypk_bytes("/shared", SCRIPT_BUNDLE_YPK)
        .expect("shared_scripts.ypk must mount");
}

#[derive(Copy, Clone, PartialEq)]
pub enum GameType {
    PAL3,
    PAL3A,
    PAL4,
    PAL5,
    PAL5Q,
    SWD5,
    SWDHC,
    SWDCF,
    Gujian,
    Gujian2,
}

impl GameType {
    pub fn app_name(&self) -> &'static str {
        match self {
            GameType::PAL3 => "OpenPAL3",
            GameType::PAL3A => "OpenPAL3A",
            GameType::PAL4 => "OpenPAL4",
            GameType::PAL5 => "OpenPAL5",
            GameType::PAL5Q => "OpenPAL5Q",
            GameType::SWD5 => "OpenSWD5",
            GameType::SWDHC => "OpenSWDHC",
            GameType::SWDCF => "OpenSWDCF",
            GameType::Gujian => "OpenGujian",
            GameType::Gujian2 => "OpenGujian2",
        }
    }

    pub fn full_name(&self) -> &'static str {
        match self {
            GameType::PAL3 => "仙剑奇侠传三",
            GameType::PAL3A => "仙剑奇侠传三外传",
            GameType::PAL4 => "仙剑奇侠传四",
            GameType::PAL5 => "仙剑奇侠传五",
            GameType::PAL5Q => "仙剑奇侠传五前传",
            GameType::SWD5 => "轩辕剑五",
            GameType::SWDHC => "轩辕剑外传 汉之云",
            GameType::SWDCF => "轩辕剑外传 云之遥",
            GameType::Gujian => "古剑奇谭",
            GameType::Gujian2 => "古剑奇谭二",
        }
    }

    /// Asset-relative paths of the fonts shipped with the game, in
    /// priority order. The first one that exists is used for in-game text;
    /// an empty list (or none found) falls back to the bundled font.
    ///
    /// PAL3 text is decoded as GBK (simplified), so `simsun.ttc` is the
    /// right face; the traditional `mingliu.ttc` is intentionally omitted.
    /// PAL4 prefers the kai face the original uses, falling back to simsun.
    /// PAL5 ships no font (uses the OS system font), so it falls back to
    /// the bundled font here.
    pub fn ui_font_candidates(&self) -> &'static [&'static str] {
        match self {
            GameType::PAL3 => &["simsun.ttc"],
            GameType::PAL3A => &["simsun.ttc"],
            GameType::PAL4 => &["gamedata/ui/fonts/kai.TTF", "gamedata/ui/fonts/simsun.ttc"],
            _ => &[],
        }
    }

    /// Extra size multiplier applied to the game-shipped UI font, on top of
    /// the per-face ideograph normalization done in `ImguiContext`. It
    /// expresses how large the *original* game renders its dialog text
    /// relative to the bundled-font baseline. Measured from in-game vs.
    /// original comparison screenshots, both PAL3 and PAL4 render ~1.5x
    /// larger than the bundled baseline. Games without a shipped font return
    /// `1.0` (no effect).
    pub fn ui_font_scale(&self) -> f32 {
        match self {
            GameType::PAL3 => 1.51,
            GameType::PAL3A => 1.51,
            GameType::PAL4 => 1.49,
            _ => 1.0,
        }
    }

    pub fn dff_loader_config(&self) -> Option<&DffLoaderConfig<'_>> {
        match self {
            GameType::PAL3 => None,
            GameType::PAL3A => None,
            GameType::PAL4 => Some(&PAL4_DFF_LOADER_CONFIG),
            GameType::PAL5 => Some(&PAL5_DFF_LOADER_CONFIG),
            GameType::PAL5Q => Some(&PAL5_DFF_LOADER_CONFIG),
            GameType::SWD5 => Some(&SWD5_DFF_LOADER_CONFIG),
            GameType::SWDHC => Some(&SWD5_DFF_LOADER_CONFIG),
            GameType::SWDCF => Some(&SWD5_DFF_LOADER_CONFIG),
            GameType::Gujian => None,
            GameType::Gujian2 => None,
        }
    }

    pub fn config_key(&self) -> &'static str {
        match self {
            GameType::PAL3 => "pal3",
            GameType::PAL3A => "pal3a",
            GameType::PAL4 => "pal4",
            GameType::PAL5 => "pal5",
            GameType::PAL5Q => "pal5q",
            GameType::SWD5 => "swd5",
            GameType::SWDHC => "swdhc",
            GameType::SWDCF => "swdcf",
            GameType::Gujian => "gujian",
            GameType::Gujian2 => "gujian2",
        }
    }

    pub fn from_config_key(key: &str) -> Option<Self> {
        match key {
            "pal3" => Some(GameType::PAL3),
            "pal3a" => Some(GameType::PAL3A),
            "pal4" => Some(GameType::PAL4),
            "pal5" => Some(GameType::PAL5),
            "pal5q" => Some(GameType::PAL5Q),
            "swd5" => Some(GameType::SWD5),
            "swdhc" => Some(GameType::SWDHC),
            "swdcf" => Some(GameType::SWDCF),
            "gujian" => Some(GameType::Gujian),
            "gujian2" => Some(GameType::Gujian2),
            _ => None,
        }
    }

    /// Decryption key used by `packfs::init_virtual_fs` for `.pkg`
    /// archives shipped by certain games. `None` for games that
    /// either don't use `.pkg` at all or whose `.pkg` files are not
    /// encrypted. Single source of truth — both the `yaobow` binary
    /// (per-game launch path) and `yaobow_editor` consume this.
    pub fn pkg_key(&self) -> Option<&'static str> {
        match self {
            GameType::PAL5 => Some("Y%H^uz6i"),
            GameType::PAL5Q => Some("L#Z^zyjq"),
            _ => None,
        }
    }

    pub fn all() -> &'static [GameType] {
        &[
            GameType::PAL3,
            GameType::PAL3A,
            GameType::PAL4,
            GameType::PAL5,
            GameType::PAL5Q,
            GameType::SWD5,
            GameType::SWDHC,
            GameType::SWDCF,
            GameType::Gujian,
            GameType::Gujian2,
        ]
    }
}

/// Read the first existing game-shipped UI font for `game` from
/// `asset_path` (see [`GameType::ui_font_candidates`]). Returns the raw
/// TTF/TTC bytes, or `None` when the game ships no font (PAL5) or none of
/// the candidate files are present — callers then keep the bundled font.
///
/// Fonts live as plain files alongside the game install (PAL3 root,
/// PAL4 `gamedata/ui/fonts/`), so they're read straight from disk rather
/// than the asset VFS.
pub fn load_game_font(game: GameType, asset_path: &str) -> Option<Vec<u8>> {
    for rel in game.ui_font_candidates() {
        let path = std::path::Path::new(asset_path).join(rel);
        match std::fs::read(&path) {
            Ok(bytes) => {
                log::info!("Using game-shipped font {}", path.display());
                return Some(bytes);
            }
            Err(err) => {
                log::debug!("game font candidate {} unavailable: {err}", path.display());
            }
        }
    }
    None
}

lazy_static::lazy_static! {
    static ref PAL4_DFF_LOADER_CONFIG: DffLoaderConfig::<'static> = DffLoaderConfig {
        texture_resolver: &Pal4TextureResolver {},
        keep_right_to_render_only: true,
        force_unique_materials: false,
        ignore_root_frame_translation: false,

        bsp_lightmap_tint: None,
        dynamic_lighting: false,
        fog_exempt: false,
    };

    static ref PAL5_DFF_LOADER_CONFIG: DffLoaderConfig::<'static> = DffLoaderConfig {
        texture_resolver: &Pal5TextureResolver {},
        keep_right_to_render_only: false,
        force_unique_materials: false,
        ignore_root_frame_translation: false,

        bsp_lightmap_tint: None,
        dynamic_lighting: true,
        fog_exempt: false,
    };

    static ref SWD5_DFF_LOADER_CONFIG: DffLoaderConfig::<'static> = DffLoaderConfig {
        texture_resolver: &Swd5TextureResolver {},
        keep_right_to_render_only: false,
        force_unique_materials: false,
        ignore_root_frame_translation: false,

        bsp_lightmap_tint: None,
        dynamic_lighting: false,
        fog_exempt: false,
    };
}
