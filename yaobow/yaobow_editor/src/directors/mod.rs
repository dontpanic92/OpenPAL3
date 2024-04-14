pub mod main_content;
pub mod main_director;
pub mod welcome_page;

use std::rc::Rc;

use crosscom::ComRc;
pub use main_director::DevToolsDirector;
use radiance::{comdef::IEntity, rendering::ComponentFactory};

#[allow(dead_code)]
#[derive(Clone)]
pub enum DevToolsState {
    MainWindow,
    PreviewEntity(ComRc<IEntity>),
    PreviewScene { cpk_name: String, scn_name: String },
}

#[derive(Clone)]
pub enum DevToolsAssetLoader {
    Pal3(Rc<shared::openpal3::asset_manager::AssetManager>),
    Pal4(Rc<shared::openpal4::asset_loader::AssetLoader>),
    Pal5(Rc<shared::openpal5::asset_loader::AssetLoader>),
    Swd5(Rc<shared::openswd5::asset_loader::AssetLoader>),
}

impl DevToolsAssetLoader {
    pub fn pal3(&self) -> Option<Rc<shared::openpal3::asset_manager::AssetManager>> {
        match self {
            DevToolsAssetLoader::Pal3(asset_mgr) => Some(asset_mgr.clone()),
            _ => None,
        }
    }

    pub fn pal4(&self) -> Option<Rc<shared::openpal4::asset_loader::AssetLoader>> {
        match self {
            DevToolsAssetLoader::Pal4(asset_mgr) => Some(asset_mgr.clone()),
            _ => None,
        }
    }

    pub fn pal5(&self) -> Option<Rc<shared::openpal5::asset_loader::AssetLoader>> {
        match self {
            DevToolsAssetLoader::Pal5(asset_mgr) => Some(asset_mgr.clone()),
            _ => None,
        }
    }

    pub fn swd5(&self) -> Option<Rc<shared::openswd5::asset_loader::AssetLoader>> {
        match self {
            DevToolsAssetLoader::Swd5(asset_mgr) => Some(asset_mgr.clone()),
            _ => None,
        }
    }

    pub fn component_factory(&self) -> Rc<dyn ComponentFactory> {
        match self {
            DevToolsAssetLoader::Pal3(asset_mgr) => asset_mgr.component_factory(),
            DevToolsAssetLoader::Pal4(asset_mgr) => asset_mgr.component_factory(),
            DevToolsAssetLoader::Pal5(asset_mgr) => asset_mgr.component_factory(),
            DevToolsAssetLoader::Swd5(asset_mgr) => asset_mgr.component_factory(),
        }
    }

    pub fn vfs(&self) -> &mini_fs::MiniFs {
        match self {
            DevToolsAssetLoader::Pal3(asset_mgr) => asset_mgr.vfs(),
            DevToolsAssetLoader::Pal4(asset_mgr) => asset_mgr.vfs(),
            DevToolsAssetLoader::Pal5(asset_mgr) => asset_mgr.vfs(),
            DevToolsAssetLoader::Swd5(asset_mgr) => asset_mgr.vfs(),
        }
    }
}
