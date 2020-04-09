use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::Mv3ModelEntity;
use crate::scene::ScnScene;
use imgui::Ui;
use radiance::scene::CoreScene;

#[derive(Clone)]
pub struct SceCommandRunScriptMode {
    mode: i32,
}

impl SceCommand for SceCommandRunScriptMode {
    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        state.set_run_mode(self.mode);
        return true;
    }
}

impl SceCommandRunScriptMode {
    pub fn new(mode: i32) -> Self {
        Self { mode }
    }
}
