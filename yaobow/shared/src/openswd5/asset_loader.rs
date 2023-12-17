use std::{io::Cursor, rc::Rc};

use common::store_ext::StoreExt2;
use fileformats::{binrw::BinRead, c00::C00};
use mini_fs::MiniFs;
use radiance::rendering::ComponentFactory;

use crate::GameType;

pub struct AssetLoader {
    game: GameType,
    vfs: MiniFs,
    // component_factory: Rc<dyn ComponentFactory>,
}

impl AssetLoader {
    pub fn new(
        // component_factory: Rc<dyn ComponentFactory>,
        vfs: MiniFs,
        game: GameType,
    ) -> Rc<Self> {
        Rc::new(Self {
            game,
            vfs,
            // component_factory,
        })
    }

    pub fn load_main_script(&self) -> anyhow::Result<Vec<u8>> {
        let content = self.vfs.read_to_end(self.main_script_path())?;
        let mut reader = Cursor::new(content);
        let c00 = C00::read(&mut reader)?;

        let lzo: minilzo_rs::LZO = minilzo_rs::LZO::init()?;
        let out = lzo.decompress(&c00.data, c00.header.original_size as usize)?;

        Ok(out)
    }

    fn main_script_path(&self) -> String {
        match self.game {
            GameType::SWD5 => "/Script/0000.C01".to_string(),
            GameType::SWDHC => "/Text/main/0000.C01".to_string(),
            GameType::SWDCF => "/Text/Off_Line/main/0000.C01".to_string(),
            _ => panic!("Unsupported game type"),
        }
    }
}
