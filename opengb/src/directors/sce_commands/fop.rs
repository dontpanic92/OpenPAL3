use crate::directors::sce_director::{Fop, SceCommand, SceState};
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Clone)]
pub struct SceCommandFop {
    op: i32,
}

impl SceCommand for SceCommandFop {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        match self.op {
            0 => state.fop_state_mut().reset(),
            1 => state.fop_state_mut().set_op(Fop::And),
            2 => state.fop_state_mut().set_op(Fop::Or),
            _ => panic!("Fop {} not supported", self.op),
        }

        true
    }
}

impl SceCommandFop {
    pub fn new(op: i32) -> Self {
        Self { op }
    }
}
