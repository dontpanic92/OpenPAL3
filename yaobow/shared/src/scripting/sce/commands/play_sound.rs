use std::{cell::RefCell, fmt::Debug, rc::Rc};

use crate::scripting::sce::{SceCommand, SceState};

use crosscom::ComRc;
use imgui::Ui;
use radiance::{
    audio::{AudioMemorySource, AudioSourceState, Codec},
    comdef::ISceneManager,
};

#[derive(Clone)]
pub struct SceCommandPlaySound {
    name: String,
    times: i32,
    source: Option<Rc<RefCell<Box<dyn AudioMemorySource>>>>,
}

impl Debug for SceCommandPlaySound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceCommandPlaySound")
            .field("name", &self.name)
            .field("times", &self.times)
            .finish()
    }
}

impl SceCommand for SceCommandPlaySound {
    fn initialize(&mut self, _scene_manager: ComRc<ISceneManager>, state: &mut SceState) {
        let data = state.asset_mgr().load_snd_data(&self.name);
        match data {
            Ok(d) => {
                let mut source = state.audio_engine().create_source();
                source.set_data(d, Codec::Wav);
                source.play(false);

                let source = Rc::new(RefCell::new(source));
                state.global_state_mut().add_sound_source(source.clone());

                self.source = Some(source);
            }
            Err(e) => log::error!("Cannot open sound {}: {:?}", &self.name, e),
        }
    }

    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        _state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        if self.times <= 1 {
            return true;
        }

        if self.source.is_none() {
            return true;
        }

        if self.source.as_mut().unwrap().borrow_mut().state() == AudioSourceState::Stopped {
            self.times -= 1;
            self.source.as_mut().unwrap().borrow_mut().restart();
        }

        false
    }
}

impl SceCommandPlaySound {
    pub fn new(name: String, times: i32) -> Self {
        Self {
            name,
            times,
            source: None,
        }
    }
}
