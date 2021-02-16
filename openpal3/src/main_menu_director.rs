use std::{borrow::Borrow, cell::RefCell, rc::Rc};

use imgui::{im_str, Ui};
use log::debug;
use opengb::{
    asset_manager::AssetManager,
    directors::{ExplorationDirector, PersistentState, SceDirector, SharedState},
};
use radiance::{
    audio::AudioEngine,
    input::InputEngine,
    scene::{CoreScene, DefaultScene, Director, SceneManager},
};

pub struct MainMenuDirector {
    asset_mgr: Rc<AssetManager>,
    audio_engine: Rc<dyn AudioEngine>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
}

impl MainMenuDirector {
    pub fn new(
        asset_mgr: Rc<AssetManager>,
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
    ) -> Self {
        Self {
            asset_mgr,
            audio_engine,
            input_engine,
        }
    }
}

impl Director for MainMenuDirector {
    fn activate(&mut self, scene_manager: &mut dyn SceneManager) {
        debug!("MainMenuDirector activated");
        scene_manager.push_scene(Box::new(DefaultScene::create()));
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        if ui.button(im_str!("开始游戏"), [120., 40.]) {
            // let scene = Box::new(CoreScene::new(self.asset_mgr.load_scn("Q01", "yn09a")));
            // scene_manager.push_scene(scene);

            let p_state = Rc::new(RefCell::new(PersistentState::new("OpenPAL3".to_string())));
            let shared_state = Rc::new(RefCell::new(SharedState::new(
                self.asset_mgr.clone(),
                self.audio_engine.borrow(),
                p_state,
            )));
            let sce_director = SceDirector::new(
                self.audio_engine.clone(),
                self.input_engine.clone(),
                self.asset_mgr.load_init_sce(),
                self.asset_mgr.clone(),
                shared_state,
            );
            sce_director.borrow_mut().call_proc(51);

            Some(sce_director)
        } else {
            for i in 1..5 {
                if ui.button(&im_str!("存档 {}", i), [120., 40.]) {
                    let p_state = PersistentState::load("OpenPAL3", i);
                    let scene_name = p_state.scene_name();
                    let sub_scene_name = p_state.sub_scene_name();
                    let bgm_name = p_state.bgm_name();
                    if scene_name.is_none() || sub_scene_name.is_none() {
                        log::error!("Cannot load save {}: scene or sub_scene is empty", i);
                        return None;
                    }

                    let scene = Box::new(CoreScene::new(self.asset_mgr.load_scn(
                        scene_name.as_ref().unwrap(),
                        sub_scene_name.as_ref().unwrap(),
                    )));
                    scene_manager.push_scene(scene);

                    let shared_state = Rc::new(RefCell::new(SharedState::new(
                        self.asset_mgr.clone(),
                        self.audio_engine.borrow(),
                        Rc::new(RefCell::new(p_state)),
                    )));

                    if let Some(bgm) = bgm_name {
                        shared_state.borrow_mut().play_bgm(&bgm);
                    }

                    let sce_director = SceDirector::new(
                        self.audio_engine.clone(),
                        self.input_engine.clone(),
                        self.asset_mgr.load_sce(scene_name.as_ref().unwrap()),
                        self.asset_mgr.clone(),
                        shared_state.clone(),
                    );

                    return Some(Rc::new(RefCell::new(ExplorationDirector::new(
                        sce_director,
                        self.input_engine.clone(),
                        shared_state,
                    ))));
                }
            }

            None
        }
    }
}
