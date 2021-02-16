use crate::directors::sce_director::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Clone)]
pub struct SceCommandStopMusic {}

impl SceCommand for SceCommandStopMusic {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        state.shared_state_mut().bgm_source().stop();
        true
    }
}

impl SceCommandStopMusic {
    pub fn new() -> Self {
        Self {}
    }
}
