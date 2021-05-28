use std::{cell::RefCell, rc::Rc};

use imgui::{im_str, Ui};
use log::debug;
use opengb::{asset_manager::AssetManager, directors::AdventureDirector};
use radiance::{
    audio::{AudioEngine, AudioSource, Codec},
    input::InputEngine,
    scene::{DefaultScene, Director, SceneManager},
};

pub struct MainMenuDirector {
    asset_mgr: Rc<AssetManager>,
    audio_engine: Rc<dyn AudioEngine>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
    main_theme_source: Box<dyn AudioSource>,
}

impl MainMenuDirector {
    pub fn new(
        asset_mgr: Rc<AssetManager>,
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
    ) -> Self {
        let data = asset_mgr.load_music_data("PI01");
        let mut main_theme_source = audio_engine.create_source();
        main_theme_source.play(data, Codec::Mp3, true);

        Self {
            asset_mgr,
            audio_engine,
            input_engine,
            main_theme_source,
        }
    }
}

impl Director for MainMenuDirector {
    fn activate(&mut self, scene_manager: &mut dyn SceneManager) {
        debug!("MainMenuDirector activated");
        scene_manager.push_scene(Box::new(DefaultScene::create()));
        self.main_theme_source.restart();
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        self.main_theme_source.update();

        if ui.button(im_str!("开始游戏"), [120., 40.]) {
            return Some(Rc::new(RefCell::new(AdventureDirector::new(
                "OpenPAL3",
                self.asset_mgr.clone(),
                self.audio_engine.clone(),
                self.input_engine.clone(),
            ))));
        } else {
            for i in 1..5 {
                if ui.button(&im_str!("存档 {}", i), [120., 40.]) {
                    let director = AdventureDirector::load(
                        "OpenPAL3",
                        self.asset_mgr.clone(),
                        self.audio_engine.clone(),
                        self.input_engine.clone(),
                        scene_manager,
                        i,
                    );

                    return Some(Rc::new(RefCell::new(director.unwrap())));
                }
            }

            None
        }
    }
}
