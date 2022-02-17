use crate::input::{engine::AxisState, KeyState};

pub struct PspInput {}

impl PspInput {
    pub fn new() -> Self {
        Self {}
    }

    pub fn process_message(&mut self, states: &mut [KeyState], axis_states: &mut [AxisState]) {}
}
