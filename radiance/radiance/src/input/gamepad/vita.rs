use crate::input::{AxisState, KeyState};

pub struct VitaGamepadInput;

impl VitaGamepadInput {
    pub fn new() -> Self {
        Self {}
    }

    pub fn process_message(&mut self, states: &mut [KeyState], axis_states: &mut [AxisState]) {}
}