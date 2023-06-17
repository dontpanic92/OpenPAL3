use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandIdle {
    idle_sec: f32,
    cur_sec: f32,
}

impl SceCommand for SceCommandIdle {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        self.cur_sec += delta_sec;
        self.cur_sec > self.idle_sec
    }
}

impl SceCommandIdle {
    pub fn new(idle_sec: f32) -> Self {
        Self {
            idle_sec,
            cur_sec: 0.,
        }
    }
}
