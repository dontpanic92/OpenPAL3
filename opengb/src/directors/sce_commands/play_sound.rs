use crate::directors::sce_director::SceCommand;
use crate::directors::sce_state::SceState;
use imgui::Ui;
use radiance::{
    audio::{AudioSourceState, Codec},
    scene::SceneManager,
};

#[derive(Clone)]
pub struct SceCommandPlaySound {
    name: String,
    times: i32,
}

impl SceCommand for SceCommandPlaySound {
    fn initialize(&mut self, scene_manager: &mut dyn SceneManager, state: &mut SceState) {
        let data = state.asset_mgr().load_snd_data(&self.name);
        state.sound_source().play(data, Codec::Wav, false);
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
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
    pub fn new(name: String, times: i32) -> Self {
        Self { name, times }
    }
}
