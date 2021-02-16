use crate::directors::sce_director::{SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;
use rand::{rngs::ThreadRng, Rng};

#[derive(Clone)]
pub struct SceCommandRnd {
    var: i16,
    max_value: i32,
    rng: ThreadRng,
}

impl SceCommand for SceCommandRnd {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let value = self.rng.gen_range(0..self.max_value);
        if self.var < 0 {
            state
                .shared_state_mut()
                .persistent_state_mut()
                .set_global(self.var, value)
        } else {
            state.vm_context_mut().set_local(self.var, value)
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
