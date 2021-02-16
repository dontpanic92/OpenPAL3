use super::engine::{InputEngine, InputEngineInternal, Key, KeyState};
use crate::application::Platform;
use std::{
    cell::RefCell,
    mem::swap,
    rc::{Rc, Weak},
};
use winapi::um::winuser;

pub struct WindowsInputEngine {
    input_engine: Weak<RefCell<WindowsInputEngine>>,
    last_key_states: Box<Vec<KeyState>>,
    key_states: Box<Vec<KeyState>>,
}

impl WindowsInputEngine {
    pub fn new(platform: &mut Platform) -> Rc<RefCell<WindowsInputEngine>> {
        let engine = Rc::new(RefCell::new(WindowsInputEngine {
            input_engine: Weak::new(),
            last_key_states: Box::new(vec![
                KeyState::new(false, false, false);
                Key::Unknown as usize
            ]),
            key_states: Box::new(vec![
                KeyState::new(false, false, false);
                Key::Unknown as usize
            ]),
        }));

        engine.borrow_mut().input_engine = Rc::downgrade(&engine);
        Self::append_message_callback_to(engine.clone(), platform);
        engine
    }

    fn append_message_callback_to(_self: Rc<RefCell<Self>>, platform: &mut Platform) {
        platform.add_message_callback(Box::new(move |msg| {
            _self.borrow_mut().message_callback(msg)
        }));
    }

    fn message_callback(&mut self, msg: &winuser::MSG) {
        let mut action: Box<dyn FnMut(Key)>;
        match msg.message {
            winuser::WM_KEYDOWN => {
                // The 31 lsb == 0 represents the key was up before this WM_KEYDOWN
                let pressed = (msg.lParam & 0x40000000) == 0;
                action = Box::new(move |key| {
                    self.last_key_states[key as usize].set_down(true);
                    self.last_key_states[key as usize].set_pressed(pressed);
                });
            }
            winuser::WM_KEYUP => {
                action = Box::new(|key| {
                    self.last_key_states[key as usize].set_down(false);
                    self.last_key_states[key as usize].set_released(true);
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

impl InputEngine for WindowsInputEngine {
    fn get_key_state(&self, key: Key) -> KeyState {
        self.key_states[key as usize]
    }
}

impl InputEngineInternal for WindowsInputEngine {
    fn update(&mut self, delta_sec: f32) {
        swap(&mut self.key_states, &mut self.last_key_states);
        for (next_state, cur_state) in self
            .last_key_states
            .iter_mut()
            .zip(self.key_states.iter_mut())
        {
            next_state.reset_action();
            next_state.set_down(cur_state.is_down());
        }
    }

    fn as_input_engine(&self) -> Rc<RefCell<dyn InputEngine>> {
        self.input_engine.upgrade().unwrap()
    }
}
