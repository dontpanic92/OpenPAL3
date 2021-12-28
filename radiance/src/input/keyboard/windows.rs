use crate::input::{Key, KeyState};
use winapi::um::winuser;

pub struct KeyboardInput;

impl KeyboardInput {
    pub fn process_message(&mut self, states: &mut [KeyState], msg: &winuser::MSG) {
        let mut action: Box<dyn FnMut(Key)>;
        match msg.message {
            winuser::WM_KEYDOWN => {
                // The 31 lsb == 0 represents the key was up before this WM_KEYDOWN
                let pressed = (msg.lParam & 0x40000000) == 0;
                action = Box::new(move |key| {
                    states[key as usize].set_down(true);
                    states[key as usize].set_pressed(pressed);
                });
            }
            winuser::WM_KEYUP => {
                action = Box::new(|key| {
                    states[key as usize].set_down(false);
                    states[key as usize].set_released(true);
                });
            }
            _ => return,
        }

        let key = match msg.wParam as i32 {
            0x30 => Key::Num0,
            0x31 => Key::Num1,
            0x32 => Key::Num2,
            0x33 => Key::Num3,
            0x34 => Key::Num4,
            0x35 => Key::Num5,
            0x36 => Key::Num6,
            0x37 => Key::Num7,
            0x38 => Key::Num8,
            0x39 => Key::Num9,
            0x41 => Key::A,
            0x42 => Key::B,
            0x43 => Key::C,
            0x44 => Key::D,
            0x45 => Key::E,
            0x46 => Key::F,
            0x47 => Key::G,
            0x48 => Key::H,
            0x49 => Key::I,
            0x4A => Key::J,
            0x4B => Key::K,
            0x4C => Key::L,
            0x4D => Key::M,
            0x4E => Key::N,
            0x4F => Key::O,
            0x50 => Key::P,
            0x51 => Key::Q,
            0x52 => Key::R,
            0x53 => Key::S,
            0x54 => Key::T,
            0x55 => Key::U,
            0x56 => Key::V,
            0x57 => Key::W,
            0x58 => Key::X,
            0x59 => Key::Y,
            0x5A => Key::Z,
            winuser::VK_OEM_3 => Key::Tilde,
            winuser::VK_UP => Key::Up,
            winuser::VK_DOWN => Key::Down,
            winuser::VK_LEFT => Key::Left,
            winuser::VK_RIGHT => Key::Right,
            winuser::VK_SPACE => Key::Space,
            _ => return,
        };

        action(key);
    }
}
