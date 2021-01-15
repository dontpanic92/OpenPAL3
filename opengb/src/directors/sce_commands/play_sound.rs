use std::{cell::RefCell, rc::Rc};

use crate::directors::sce_director::{SceCommand, SceState};

use imgui::Ui;
use radiance::{
    audio::{AudioSource, AudioSourceState, Codec},
    scene::SceneManager,
};

#[derive(Clone)]
pub struct SceCommandPlaySound {
    name: String,
    times: i32,
    source: Option<Rc<RefCell<Box<dyn AudioSource>>>>,
}

impl SceCommand for SceCommandPlaySound {
    fn initialize(&mut self, scene_manager: &mut dyn SceneManager, state: &mut SceState) {
        let data = state.asset_mgr().load_snd_data(&self.name);
        let mut source = state.audio().create_source();
        source.play(data, Codec::Wav, false);

        let source = Rc::new(RefCell::new(source));
        state.shared_state_mut().add_sound_source(source.clone());

        self.source = Some(source);
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        if self.times <= 1 {
            return true;
        }

        if self.source.as_mut().unwrap().borrow_mut().state() == AudioSourceState::Stopped {
            println!("in cmd: stopped");
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
