use crate::asset_manager::AssetManager;
use radiance::{
    audio::{AudioEngine, AudioSource},
    input::InputEngine,
};
use std::{
    any::Any,
    cell::{Ref, RefCell, RefMut},
    ops::Deref,
};
use std::{collections::HashMap, rc::Rc};

use super::shared_state::SharedState;

pub struct SceState {
    asset_mgr: Rc<AssetManager>,
    shared_state: Rc<RefCell<SharedState>>,
    run_mode: i32,
    ext: HashMap<String, Box<dyn Any>>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
}

impl SceState {
    pub fn new(
        audio_engine: &dyn AudioEngine,
        input_engine: Rc<RefCell<dyn InputEngine>>,
        asset_mgr: Rc<AssetManager>,
        shared_state: Rc<RefCell<SharedState>>,
    ) -> Self {
        let ext = HashMap::<String, Box<dyn Any>>::new();

        Self {
            asset_mgr: asset_mgr.clone(),
            shared_state,
            run_mode: 1,
            ext,
            input_engine,
        }
    }

    pub fn shared_state_mut(&mut self) -> RefMut<SharedState> {
        self.shared_state.borrow_mut()
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
