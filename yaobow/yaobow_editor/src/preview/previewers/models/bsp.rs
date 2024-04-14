use std::{io::Read, path::Path};

use crosscom::ComRc;
use fileformats::rwbs::read_bsp;
use mini_fs::{MiniFs, StoreExt};
use radiance::comdef::IEntity;
use shared::{loaders::bsp::create_entity_from_bsp_model, GameType};

use crate::{
    directors::DevToolsAssetLoader,
    preview::previewers::{get_extension, jsonify},
};

use super::ModelLoader;

pub struct BspModelLoader {
    asset_mgr: DevToolsAssetLoader,
    game_type: GameType,
}

impl BspModelLoader {
    pub fn new(asset_mgr: DevToolsAssetLoader, game_type: GameType) -> Self {
        Self {
            asset_mgr,
            game_type,
        }
    }
}

impl ModelLoader for BspModelLoader {
    fn load_text(&self, vfs: &MiniFs, path: &Path) -> String {
        let mut buf = vec![];
        _ = vfs.open(&path).unwrap().read_to_end(&mut buf);
        read_bsp(&buf)
            .map(|f| jsonify(&f))
            .unwrap_or("Unsupported".to_string())
    }

    fn is_supported(&self, path: &Path) -> bool {
        let extension = get_extension(path);
        extension.as_deref() == Some("bsp")
    }

    fn load(&self, vfs: &MiniFs, path: &Path) -> ComRc<IEntity> {
        create_entity_from_bsp_model(
            &self.asset_mgr.component_factory(),
            vfs,
            path,
            "preview".to_string(),
            self.game_type.dff_loader_config().unwrap(),
        )
    }
}
