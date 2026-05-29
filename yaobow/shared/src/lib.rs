use loaders::{dff::DffLoaderConfig, Pal4TextureResolver};

use crate::loaders::{Pal5TextureResolver, Swd5TextureResolver};

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
pub mod playground;
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

lazy_static::lazy_static! {
    static ref PAL4_DFF_LOADER_CONFIG: DffLoaderConfig::<'static> = DffLoaderConfig {
        texture_resolver: &Pal4TextureResolver {},
        keep_right_to_render_only: true,
        force_unique_materials: false,
        ignore_root_frame_translation: false,

        bsp_lightmap_tint: None,
    };

    static ref PAL5_DFF_LOADER_CONFIG: DffLoaderConfig::<'static> = DffLoaderConfig {
        texture_resolver: &Pal5TextureResolver {},
        keep_right_to_render_only: false,
        force_unique_materials: false,
        ignore_root_frame_translation: false,

        bsp_lightmap_tint: None,
    };

    static ref SWD5_DFF_LOADER_CONFIG: DffLoaderConfig::<'static> = DffLoaderConfig {
        texture_resolver: &Swd5TextureResolver {},
        keep_right_to_render_only: false,
        force_unique_materials: false,
        ignore_root_frame_translation: false,

        bsp_lightmap_tint: None,
    };
}
