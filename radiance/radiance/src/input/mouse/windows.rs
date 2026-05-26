use crate::input::{KeyState, MouseButton};
use winapi::shared::{minwindef::HIWORD, windef::POINT};
use winapi::um::winuser;

const WHEEL_DELTA: f32 = 120.0;

pub struct MouseInput {
    last_cursor: Option<POINT>,
}

impl MouseInput {
    pub fn new() -> Self {
        Self { last_cursor: None }
    }

    pub fn process_message(
        &mut self,
        button_states: &mut [KeyState],
        delta: &mut (f32, f32),
        wheel: &mut f32,
        msg: &winuser::MSG,
    ) {
        match msg.message {
            winuser::WM_LBUTTONDOWN => set_button(button_states, MouseButton::Left, true),
            winuser::WM_LBUTTONUP => set_button(button_states, MouseButton::Left, false),
            winuser::WM_RBUTTONDOWN => set_button(button_states, MouseButton::Right, true),
            winuser::WM_RBUTTONUP => set_button(button_states, MouseButton::Right, false),
            winuser::WM_MBUTTONDOWN => set_button(button_states, MouseButton::Middle, true),
            winuser::WM_MBUTTONUP => set_button(button_states, MouseButton::Middle, false),
            winuser::WM_MOUSEMOVE => {
                // lParam packs (x, y) as signed 16-bit client coords.
                let x = (msg.lParam & 0xFFFF) as i16 as f32;
                let y = ((msg.lParam >> 16) & 0xFFFF) as i16 as f32;
                let new_pt = POINT {
                    x: x as i32,
                    y: y as i32,
                };
                if let Some(prev) = self.last_cursor {
                    delta.0 += (new_pt.x - prev.x) as f32;
                    delta.1 += (new_pt.y - prev.y) as f32;
                }
                self.last_cursor = Some(new_pt);
            }
            winuser::WM_MOUSEWHEEL => {
                // HIWORD of wParam is a signed wheel delta. WHEEL_DELTA (120)
                // == one detent; normalize so script-visible "1.0" is a notch.
                let raw = HIWORD(msg.wParam as u32) as i16 as f32;
                *wheel += raw / WHEEL_DELTA;
            }
            _ => {}
        }
    }
}

fn set_button(states: &mut [KeyState], button: MouseButton, down: bool) {
    let idx = button as usize;
    if down {
        states[idx].set_down(true);
        states[idx].set_pressed(true);
    } else {
        states[idx].set_down(false);
        states[idx].set_released(true);
    }
}
