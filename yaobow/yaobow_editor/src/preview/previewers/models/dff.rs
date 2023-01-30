use std::{io::Read, path::Path, rc::Rc};

use crosscom::ComRc;
use fileformats::dff::read_dff;
use mini_fs::{MiniFs, StoreExt};
use opengb::asset_manager::AssetManager;
use radiance::comdef::IEntity;
use shared::loaders::dff::create_entity_from_dff_model;

use crate::preview::previewers::{get_extension, jsonify};

use super::ModelLoader;

pub struct DffModelLoader {
    asset_mgr: Rc<AssetManager>,
}

impl DffModelLoader {
    pub fn new(asset_mgr: Rc<AssetManager>) -> Self {
        Self { asset_mgr }
    }
}

impl ModelLoader for DffModelLoader {
    fn load_text(&self, vfs: &MiniFs, path: &Path) -> String {
        let mut buf = vec![];
        _ = vfs.open(&path).unwrap().read_to_end(&mut buf);
        read_dff(&buf)
            .map(|f| jsonify(&f))
            .unwrap_or("Unsupported".to_string())
    }

    fn is_supported(&self, path: &Path) -> bool {
        let extension = get_extension(path);
        extension.as_deref() == Some("pol")
    }

    fn load(&self, vfs: &MiniFs, path: &Path) -> ComRc<IEntity> {
        create_entity_from_dff_model(
            &self.asset_mgr.component_factory(),
            vfs,
            path,
            "preview".to_string(),
            true,
        )
    }
}
