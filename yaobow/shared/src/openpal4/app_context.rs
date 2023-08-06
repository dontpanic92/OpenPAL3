use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use radiance::{
    comdef::ISceneManager,
    input::InputEngine,
    radiance::UiManager,
    rendering::{ComponentFactory, VideoPlayer},
};

use super::asset_loader::AssetLoader;

pub struct Pal4AppContext {
    pub(crate) loader: Rc<AssetLoader>,
    pub(crate) scene_manager: ComRc<ISceneManager>,
    pub(crate) ui: Rc<UiManager>,
    pub(crate) input: Rc<RefCell<dyn InputEngine>>,

    component_factory: Rc<dyn ComponentFactory>,
    video_player: Box<VideoPlayer>,
}

impl Pal4AppContext {
    pub fn new(
        component_factory: Rc<dyn ComponentFactory>,
        loader: Rc<AssetLoader>,
        scene_manager: ComRc<ISceneManager>,
        ui: Rc<UiManager>,
        input: Rc<RefCell<dyn InputEngine>>,
    ) -> Self {
        Self {
            loader,
            scene_manager,
            ui,
            input,
            component_factory: component_factory.clone(),
            video_player: component_factory.create_video_player(),
        }
    }

    pub fn start_play_movie(&mut self, name: &str) -> Option<(u32, u32)> {
        let data = self.loader.load_video(name).unwrap();
        self.video_player.play(
            self.component_factory.clone(),
            data,
            radiance::video::Codec::Bik,
            false,
        )
    }

    pub fn video_player(&mut self) -> &mut VideoPlayer {
        &mut self.video_player
    }
}
