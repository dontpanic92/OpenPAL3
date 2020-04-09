use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::ScnScene;
use imgui::Ui;
use radiance::scene::CoreScene;

#[derive(Clone)]
pub struct SceCommandIdle {
    idle_sec: f32,
    cur_sec: f32,
}

impl SceCommand for SceCommandIdle {
    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
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
