use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::resource_manager::ResourceManager;
use imgui::Ui;
use radiance::audio::{AudioSourceState, Codec};
use radiance::scene::Scene;
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandPlaySound {
    res_man: Rc<ResourceManager>,
    times: i32,
    data: Option<Vec<u8>>,
}

impl SceCommand for SceCommandPlaySound {
    fn initialize(&mut self, scene: &mut Box<dyn Scene>, state: &mut SceState) {
        let data = self.data.take().unwrap();
        state.sound_source().play(data, Codec::Wav, false);
    }

    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        if state.sound_source().state() == AudioSourceState::Stopped {
            self.times -= 1;

            if self.times == 0 {
                return true;
            } else {
                state.sound_source().restart();
            }
        }

        false
    }
}

impl SceCommandPlaySound {
    pub fn new(res_man: &Rc<ResourceManager>, name: &str, times: i32) -> Self {
        Self {
            res_man: res_man.clone(),
            times,
            data: Some(res_man.load_snd_data(name)),
        }
    }
}
