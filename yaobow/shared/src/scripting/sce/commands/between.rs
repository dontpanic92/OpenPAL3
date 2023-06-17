use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandBetween {
    var: i16,
    lb: i32,
    hb: i32,
}

impl SceCommand for SceCommandBetween {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let lhs = if self.var < 0 {
            state
                .global_state_mut()
                .persistent_state_mut()
                .get_global(self.var)
                .unwrap_or(0)
        } else {
            state.context_mut().get_local(self.var).unwrap_or(0)
        };

        let value = (lhs >= self.lb) && (lhs <= self.hb);
        state.global_state_mut().fop_state_mut().push_value(value);
        true
    }
}

impl SceCommandBetween {
    pub fn new(var: i16, lb: i32, hb: i32) -> Self {
        Self { var, lb, hb }
    }
}
