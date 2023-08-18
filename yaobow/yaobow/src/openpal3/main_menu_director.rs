use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use imgui::{Condition, Ui};
use log::debug;
use radiance::{
    audio::{AudioEngine, AudioMemorySource, Codec},
    comdef::{IDirector, IDirectorImpl, ISceneManager},
    input::InputEngine,
    scene::CoreScene,
};
use shared::{
    openpal3::{asset_manager::AssetManager, directors::AdventureDirector},
    scripting::sce::vm::SceExecutionOptions,
};

use crate::ComObject_MainMenuDirector;

use super::sce_proc_hooks::SceRestHooks;

pub struct MainMenuDirector {
    asset_mgr: Rc<AssetManager>,
    audio_engine: Rc<dyn AudioEngine>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
    main_theme_source: RefCell<Box<dyn AudioMemorySource>>,
}

ComObject_MainMenuDirector!(super::MainMenuDirector);

impl MainMenuDirector {
    pub fn new(
        asset_mgr: Rc<AssetManager>,
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
    ) -> Self {
        let data = asset_mgr.load_music_data("PI01");
        let mut main_theme_source = audio_engine.create_source();
        main_theme_source.set_data(data, Codec::Mp3);
        main_theme_source.play(true);

        Self {
            asset_mgr,
            audio_engine,
            input_engine,
            main_theme_source: RefCell::new(main_theme_source),
        }
    }
}

impl IDirectorImpl for MainMenuDirector {
    fn activate(&self, scene_manager: ComRc<ISceneManager>) {
        debug!("MainMenuDirector activated");
        scene_manager.push_scene(CoreScene::create());
        self.main_theme_source.borrow_mut().restart();
    }

    fn update(
        &self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        _delta_sec: f32,
    ) -> Option<ComRc<IDirector>> {
        self.main_theme_source.borrow_mut().update();

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
                return Some(ComRc::from_object(AdventureDirector::new(
                    "OpenPAL3",
                    self.asset_mgr.clone(),
                    self.audio_engine.clone(),
                    self.input_engine.clone(),
                    Some(sce_options),
                )));
            } else {
                for i in 1..5 {
                    if ui.button(&format!("存档 {}", i)) {
                        let director = ComRc::from_object(
                            AdventureDirector::load(
                                "OpenPAL3",
                                self.asset_mgr.clone(),
                                self.audio_engine.clone(),
                                self.input_engine.clone(),
                                scene_manager,
                                Some(sce_options),
                                i,
                            )
                            .unwrap(),
                        );

                        return Some(director);
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
