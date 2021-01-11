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
            winuser::VK_A => Key::A,
            winuser::VK_B => Key::B,
            winuser::VK_C => Key::C,
            winuser::VK_D => Key::D,
            winuser::VK_E => Key::E,
            winuser::VK_F => Key::F,
            winuser::VK_G => Key::G,
            winuser::VK_H => Key::H,
            winuser::VK_I => Key::I,
            winuser::VK_J => Key::J,
            winuser::VK_K => Key::K,
            winuser::VK_L => Key::L,
            winuser::VK_M => Key::M,
            winuser::VK_N => Key::N,
            winuser::VK_O => Key::O,
            winuser::VK_P => Key::P,
            winuser::VK_Q => Key::Q,
            winuser::VK_R => Key::R,
            winuser::VK_S => Key::S,
            winuser::VK_T => Key::T,
            winuser::VK_U => Key::U,
            winuser::VK_V => Key::V,
            winuser::VK_W => Key::W,
            winuser::VK_X => Key::X,
            winuser::VK_Y => Key::Y,
            winuser::VK_Z => Key::Z,
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
        for (next_state, cur_state) in self.last_key_states.iter_mut().zip(self.key_states.iter_mut()) {
            next_state.reset_action();
            next_state.set_down(cur_state.is_down());
        }
    }

    fn as_input_engine(&self) -> Rc<RefCell<dyn InputEngine>> {
        self.input_engine.upgrade().unwrap()
    }
}
