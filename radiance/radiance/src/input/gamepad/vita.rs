use crate::input::{Axis, AxisState, Key, KeyState};

pub struct VitaGamepadInput;

impl VitaGamepadInput {
    pub fn new() -> Self {
        Self {}
    }

    pub fn process_message(&mut self, states: &mut [KeyState], axis_states: &mut [AxisState]) {
        use vitasdk_sys::psp2::ctrl::*;
        use vitasdk_sys::psp2common::ctrl::SceCtrlButtons::*;
        use vitasdk_sys::psp2common::ctrl::*;
        unsafe {
            let mut ctrl: SceCtrlData = std::mem::zeroed();
            sceCtrlPeekBufferPositive(0, &mut ctrl as *mut _, 1);
            set_key_state(
                states,
                Key::GamePadEast,
                ctrl.buttons & SCE_CTRL_CIRCLE != 0,
            );
            set_key_state(
                states,
                Key::GamePadWest,
                ctrl.buttons & SCE_CTRL_SQUARE != 0,
            );
            set_key_state(
                states,
                Key::GamePadSouth,
                ctrl.buttons & SCE_CTRL_CROSS != 0,
            );
            set_key_state(
                states,
                Key::GamePadNorth,
                ctrl.buttons & SCE_CTRL_TRIANGLE != 0,
            );
            set_key_state(states, Key::GamePadDPadUp, ctrl.buttons & SCE_CTRL_UP != 0);
            set_key_state(
                states,
                Key::GamePadDPadDown,
                ctrl.buttons & SCE_CTRL_DOWN != 0,
            );
            set_key_state(
                states,
                Key::GamePadDPadLeft,
                ctrl.buttons & SCE_CTRL_LEFT != 0,
            );
            set_key_state(
                states,
                Key::GamePadDPadRight,
                ctrl.buttons & SCE_CTRL_RIGHT != 0,
            );

            axis_states[Axis::LeftStickX as usize].set_value(ctrl.lx as f32 / 255.0 * 2.0 - 1.0);
            axis_states[Axis::LeftStickY as usize].set_value(-(ctrl.ly as f32 / 255.0 * 2.0 - 1.0));
            axis_states[Axis::RightStickX as usize].set_value(ctrl.rx as f32 / 255.0 * 2.0 - 1.0);
            axis_states[Axis::RightStickY as usize]
                .set_value(-(ctrl.ry as f32 / 255.0 * 2.0 - 1.0));
        }
    }
}

fn set_key_state(states: &mut [KeyState], key: Key, down: bool) {
    if !states[key as usize].is_down() && down {
        states[key as usize].set_pressed(true);
    } else if states[key as usize].is_down() && !down {
        states[key as usize].set_released(true);
    }

    states[key as usize].set_down(down);
}
