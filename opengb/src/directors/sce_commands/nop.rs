use crate::directors::{sce_director::SceCommand, sce_state::SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Clone)]
pub struct SceCommandNop {}

impl SceCommand for SceCommandNop {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        true
    }
}

impl SceCommandNop {
    pub fn new() -> Self {
        Self {}
    }
}
