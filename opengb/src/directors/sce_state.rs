use crate::asset_manager::AssetManager;
use radiance::{
    audio::{AudioEngine, AudioSource},
    input::InputEngine,
};
use std::{
    any::Any,
    cell::{Ref, RefCell},
    ops::Deref,
};
use std::{collections::HashMap, rc::Rc};

pub struct SceState {
    asset_mgr: Rc<AssetManager>,
    bgm_source: Box<dyn AudioSource>,
    sound_source: Box<dyn AudioSource>,
    run_mode: i32,
    ext: HashMap<String, Box<dyn Any>>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
}

impl SceState {
    pub fn new(
        audio_engine: &dyn AudioEngine,
        input_engine: Rc<RefCell<dyn InputEngine>>,
        asset_mgr: Rc<AssetManager>,
    ) -> Self {
        let bgm_source = audio_engine.create_source();
        let sound_source = audio_engine.create_source();
        let ext = HashMap::<String, Box<dyn Any>>::new();

        Self {
            asset_mgr: asset_mgr.clone(),
            bgm_source,
            sound_source,
            run_mode: 1,
            ext,
            input_engine,
        }
    }

    pub fn bgm_source(&mut self) -> &mut dyn AudioSource {
        self.bgm_source.as_mut()
    }

    pub fn sound_source(&mut self) -> &mut dyn AudioSource {
        self.sound_source.as_mut()
    }

    pub fn input(&self) -> Ref<dyn InputEngine> {
        self.input_engine.borrow()
    }

    pub fn run_mode(&self) -> i32 {
        self.run_mode
    }

    pub fn set_run_mode(&mut self, run_mode: i32) {
        self.run_mode = run_mode;
    }

    pub fn ext_mut(&mut self) -> &mut HashMap<String, Box<dyn Any>> {
        &mut self.ext
    }

    pub fn asset_mgr(&self) -> &Rc<AssetManager> {
        &self.asset_mgr
    }
}
