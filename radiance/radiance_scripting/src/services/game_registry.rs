use crosscom::ComRc;

use crate::comdef::services::{IGameRegistry, IGameRegistryImpl};

#[derive(Clone, Copy)]
struct GameInfo {
    ordinal: i32,
    full_name: &'static str,
    config_key: &'static str,
    default_asset_path: &'static str,
}

const GAMES: &[GameInfo] = &[
    GameInfo {
        ordinal: 0,
        full_name: "仙剑奇侠传三",
        config_key: "OpenPAL3",
        default_asset_path: "F:\\SteamLibrary\\steamapps\\common\\PAL3",
    },
    GameInfo {
        ordinal: 1,
        full_name: "仙剑奇侠传三外传",
        config_key: "OpenPAL3A",
        default_asset_path: "F:\\SteamLibrary\\steamapps\\common\\PAL3A",
    },
    GameInfo {
        ordinal: 2,
        full_name: "仙剑奇侠传四",
        config_key: "OpenPAL4",
        default_asset_path: "F:\\SteamLibrary\\steamapps\\common\\PAL4",
    },
    GameInfo {
        ordinal: 3,
        full_name: "仙剑奇侠传五",
        config_key: "OpenPAL5",
        default_asset_path: "F:\\PAL5\\",
    },
    GameInfo {
        ordinal: 4,
        full_name: "仙剑奇侠传五前传",
        config_key: "OpenPAL5Q",
        default_asset_path: "F:\\PAL5Q\\",
    },
    GameInfo {
        ordinal: 5,
        full_name: "轩辕剑五",
        config_key: "OpenSWD5",
        default_asset_path: "F:\\SteamLibrary\\steamapps\\common\\SWD5",
    },
    GameInfo {
        ordinal: 6,
        full_name: "轩辕剑外传 汉之云",
        config_key: "OpenSWDHC",
        default_asset_path: "F:\\SteamLibrary\\steamapps\\common\\SWDHC",
    },
    GameInfo {
        ordinal: 7,
        full_name: "轩辕剑外传 云之遥",
        config_key: "OpenSWDCF",
        default_asset_path: "F:\\SteamLibrary\\steamapps\\common\\SWDCF",
    },
    GameInfo {
        ordinal: 8,
        full_name: "古剑奇谭",
        config_key: "OpenGujian",
        default_asset_path: "F:\\SteamLibrary\\steamapps\\common\\Gujian",
    },
    GameInfo {
        ordinal: 9,
        full_name: "古剑奇谭二",
        config_key: "OpenGujian2",
        default_asset_path: "F:\\SteamLibrary\\steamapps\\common\\Gujian2",
    },
];

pub struct GameRegistry;

ComObject_GameRegistry!(super::GameRegistry);

impl GameRegistry {
    pub fn create() -> ComRc<IGameRegistry> {
        ComRc::from_object(Self)
    }
}

impl IGameRegistryImpl for GameRegistry {
    fn count(&self) -> i32 {
        GAMES.len() as i32
    }

    fn game_at(&self, index: i32) -> i32 {
        GAMES.get(index as usize).map(|g| g.ordinal).unwrap_or(-1)
    }

    fn full_name(&self, game: i32) -> &str {
        find(game).map(|g| g.full_name).unwrap_or("")
    }
    fn config_key(&self, game: i32) -> &str {
        find(game).map(|g| g.config_key).unwrap_or("")
    }
    fn default_asset_path(&self, game: i32) -> &str {
        find(game).map(|g| g.default_asset_path).unwrap_or("")
    }
}

fn find(ordinal: i32) -> Option<&'static GameInfo> {
    GAMES.iter().find(|game| game.ordinal == ordinal)
}
