use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::ScnScene;
use imgui::Ui;
use radiance::scene::CoreScene;

#[derive(Clone)]
pub struct SceCommandScriptRunMode {
    mode: i32,
}

impl SceCommand for SceCommandScriptRunMode {
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

impl SceCommandScriptRunMode {
    pub fn new(mode: i32) -> Self {
        Self { mode }
    }
}
