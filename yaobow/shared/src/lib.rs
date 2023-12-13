#![feature(io_error_more)]
#![feature(cursor_remaining)]
#![feature(trait_upcasting)]

use loaders::{dff::DffLoaderConfig, Pal4TextureResolver};

use crate::loaders::{Pal5TextureResolver, Swd5TextureResolver};

pub mod config;
pub mod exporters;
pub mod fs;
pub mod loaders;
pub mod openpal3;
pub mod openpal4;
pub mod openpal5;
pub mod openswd5;
pub mod scripting;
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

    pub fn dff_loader_config(&self) -> Option<&DffLoaderConfig> {
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
}

lazy_static::lazy_static! {
    static ref PAL4_DFF_LOADER_CONFIG: DffLoaderConfig::<'static> = DffLoaderConfig {
        texture_resolver: &Pal4TextureResolver {},
        keep_right_to_render_only: true,
    };

    static ref PAL5_DFF_LOADER_CONFIG: DffLoaderConfig::<'static> = DffLoaderConfig {
        texture_resolver: &Pal5TextureResolver {},
        keep_right_to_render_only: false,
    };

    static ref SWD5_DFF_LOADER_CONFIG: DffLoaderConfig::<'static> = DffLoaderConfig {
        texture_resolver: &Swd5TextureResolver {},
        keep_right_to_render_only: false,
    };
}
