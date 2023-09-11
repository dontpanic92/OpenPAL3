mod bsp;
mod cvd;
mod dff;
mod mv3;
mod pol;

use std::{path::Path, rc::Rc};

use crosscom::ComRc;
use mini_fs::MiniFs;
use radiance::comdef::IEntity;
use shared::{openpal3::asset_manager::AssetManager, GameType};

use crate::{
    directors::{main_content::ContentTab, DevToolsState},
    preview::panes::TextPane,
};

use self::{
    bsp::BspModelLoader, cvd::CvdModelLoader, dff::DffModelLoader, mv3::Mv3ModelLoader,
    pol::PolModelLoader,
};

use super::Previewer;

pub struct ModelPreviewer {
    model_loaders: Vec<Box<dyn ModelLoader>>,
}

impl ModelPreviewer {
    pub fn new(asset_mgr: Rc<AssetManager>, game_type: GameType) -> Self {
        Self {
            model_loaders: vec![
                Box::new(Mv3ModelLoader::new(asset_mgr.clone())),
                Box::new(CvdModelLoader::new(asset_mgr.clone())),
                Box::new(DffModelLoader::new(asset_mgr.clone(), game_type)),
                Box::new(BspModelLoader::new(asset_mgr.clone(), game_type)),
                Box::new(PolModelLoader::new(asset_mgr)),
            ],
        }
    }
}

impl Previewer for ModelPreviewer {
    fn open(&self, vfs: &MiniFs, path: &Path) -> Option<ContentTab> {
        for loader in &self.model_loaders {
            if loader.is_supported(path) {
                let content = loader.load_text(vfs, path);
                return Some(ContentTab::new(
                    path.to_string_lossy().to_string(),
                    Box::new(TextPane::new(
                        content,
                        path.to_owned(),
                        Some(DevToolsState::PreviewEntity(loader.load(vfs, path))),
                        None,
                    )),
                ));
            }
        }

        None
    }
}

pub trait ModelLoader {
    fn is_supported(&self, path: &Path) -> bool;
    fn load_text(&self, vfs: &MiniFs, path: &Path) -> String;
    fn load(&self, vfs: &MiniFs, path: &Path) -> ComRc<IEntity>;
}
