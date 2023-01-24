use std::{cell::RefCell, rc::Rc};

use imgui::{Condition, Ui};
use log::debug;
use opengb::{
    asset_manager::AssetManager,
    directors::{AdventureDirector, SceExecutionOptions},
};
use radiance::{
    audio::{AudioEngine, AudioSource, Codec},
    input::InputEngine,
    scene::{CoreScene, Director, SceneManager},
};

use crate::sce_proc_hooks::SceRestHooks;

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
        scene_manager.push_scene(CoreScene::create());
        self.main_theme_source.restart();
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &Ui,
        _delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        self.main_theme_source.update();

        let sce_options = SceExecutionOptions {
            proc_hooks: vec![Box::new(SceRestHooks::new())],
        };

        let window_size = ui.io().display_size;
        let window = ui
            .window(" ")
            .size(window_size, Condition::Always)
            .position([0.0, 0.0], Condition::Always)
            .collapsible(false)
            .always_auto_resize(true);

        if let Some(Some(director)) = window.build(|| {
            if ui.button("开始游戏") {
                return Some(Rc::new(RefCell::new(AdventureDirector::new(
                    "OpenPAL3",
                    self.asset_mgr.clone(),
                    self.audio_engine.clone(),
                    self.input_engine.clone(),
                    Some(sce_options),
                ))));
            } else {
                for i in 1..5 {
                    if ui.button(&format!("存档 {}", i)) {
                        let director = AdventureDirector::load(
                            "OpenPAL3",
                            self.asset_mgr.clone(),
                            self.audio_engine.clone(),
                            self.input_engine.clone(),
                            scene_manager,
                            Some(sce_options),
                            i,
                        );

                        return Some(Rc::new(RefCell::new(director.unwrap())));
                    }
                }
                None
            }
        }) {
            Some(director)
        } else {
            None
        }
    }
}
