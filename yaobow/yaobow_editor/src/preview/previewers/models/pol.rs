use std::{io::BufReader, path::Path, rc::Rc};

use crosscom::ComRc;
use fileformats::pol::read_pol;
use mini_fs::{MiniFs, StoreExt};
use opengb::{asset_manager::AssetManager, loaders::pol::create_entity_from_pol_model};
use radiance::comdef::IEntity;

use crate::preview::previewers::{get_extension, jsonify};

use super::ModelLoader;

pub struct PolModelLoader {
    asset_mgr: Rc<AssetManager>,
}

impl PolModelLoader {
    pub fn new(asset_mgr: Rc<AssetManager>) -> Self {
        Self { asset_mgr }
    }
}

impl ModelLoader for PolModelLoader {
    fn load_text(&self, vfs: &MiniFs, path: &Path) -> String {
        read_pol(&mut BufReader::new(vfs.open(&path).unwrap()))
            .map(|f| jsonify(&f))
            .unwrap_or("Unsupported".to_string())
    }

    fn is_supported(&self, path: &Path) -> bool {
        let extension = get_extension(path);
        extension.as_deref() == Some("pol")
    }

    fn load(&self, vfs: &MiniFs, path: &Path) -> ComRc<IEntity> {
        create_entity_from_pol_model(
            &self.asset_mgr.component_factory(),
            vfs,
            path,
            "preview".to_string(),
            true,
        )
    }
}
