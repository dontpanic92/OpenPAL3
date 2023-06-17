use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

use crate::{
    openpal3::states::global_state::Fop,
    scripting::sce::{SceCommand, SceState},
};

#[derive(Debug, Clone)]
pub struct SceCommandFop {
    op: i32,
}

impl SceCommand for SceCommandFop {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        match self.op {
            0 => state.global_state_mut().fop_state_mut().reset(),
            1 => state.global_state_mut().fop_state_mut().set_op(Fop::And),
            2 => state.global_state_mut().fop_state_mut().set_op(Fop::Or),
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
