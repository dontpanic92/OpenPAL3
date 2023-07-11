use std::rc::Rc;

use crosscom::ComRc;
use radiance::{comdef::ISceneManager, radiance::UiManager};

use super::asset_loader::AssetLoader;

pub struct Pal4AppContext {
    pub(crate) loader: Rc<AssetLoader>,
    pub(crate) scene_manager: ComRc<ISceneManager>,
    pub(crate) ui: Rc<UiManager>,
}

impl Pal4AppContext {
    pub fn new(
        loader: Rc<AssetLoader>,
        scene_manager: ComRc<ISceneManager>,
        ui: Rc<UiManager>,
    ) -> Self {
        Self {
            loader,
            scene_manager,
            ui,
        }
    }
}
