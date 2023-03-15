use std::{path::Path, rc::Rc};

use crosscom::ComRc;
use mini_fs::MiniFs;
use opengb::{
    asset_manager::AssetManager, loaders::cvd_loader::cvd_load_from_file,
    scene::create_entity_from_cvd_model,
};
use radiance::comdef::IEntity;

use crate::preview::previewers::{get_extension, jsonify};

use super::ModelLoader;

pub struct CvdModelLoader {
    asset_mgr: Rc<AssetManager>,
}

impl CvdModelLoader {
    pub fn new(asset_mgr: Rc<AssetManager>) -> Self {
        Self { asset_mgr }
    }
}

impl ModelLoader for CvdModelLoader {
    fn load_text(&self, vfs: &MiniFs, path: &Path) -> String {
        cvd_load_from_file(vfs, path)
            .map(|f| jsonify(&f))
            .unwrap_or("Unsupported".to_string())
    }

    fn is_supported(&self, path: &Path) -> bool {
        let extension = get_extension(path);
        extension.as_deref() == Some("cvd")
    }

    fn load(&self, vfs: &MiniFs, path: &Path) -> ComRc<IEntity> {
        create_entity_from_cvd_model(
            self.asset_mgr.component_factory(),
            vfs,
            path,
            "preview".to_string(),
            true,
        )
    }
}
