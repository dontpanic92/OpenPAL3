use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;
use rand::{rngs::ThreadRng, Rng};

#[derive(Debug, Clone)]
pub struct SceCommandRnd {
    var: i16,
    max_value: i32,
    rng: ThreadRng,
}

impl SceCommand for SceCommandRnd {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let value = self.rng.gen_range(0..self.max_value);
        if self.var < 0 {
            state
                .global_state_mut()
                .persistent_state_mut()
                .set_global(self.var, value)
        } else {
            state.context_mut().set_local(self.var, value)
        }

        true
    }
}

impl SceCommandRnd {
    pub fn new(var: i16, max_value: i32) -> Self {
        Self {
            var,
            max_value,
            rng: rand::thread_rng(),
        }
    }
}
