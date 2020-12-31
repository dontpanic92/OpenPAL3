use super::engine::{InputEngine, InputEngineInternal, Key, KeyState};
use crate::application::Platform;
use std::{cell::RefCell, rc::Rc};
use winapi::um::winuser;

pub struct WindowsInputEngine {
    last_key_states: RefCell<Box<Vec<KeyState>>>,
    key_states: RefCell<Box<Vec<KeyState>>>,
}

impl WindowsInputEngine {
    pub fn new(platform: &mut Platform) -> Rc<WindowsInputEngine> {
        let engine = Rc::new(WindowsInputEngine {
            last_key_states: RefCell::new(Box::new(vec![
                KeyState::new(false, false, false);
                Key::Unknown as usize
            ])),
            key_states: RefCell::new(Box::new(vec![
                KeyState::new(false, false, false);
                Key::Unknown as usize
            ])),
        });

        engine.append_message_callback_to(platform);
        engine
    }

    fn append_message_callback_to(self: &Rc<Self>, platform: &mut Platform) {
        let engine = self.clone();
        platform.add_message_callback(Box::new(move |msg| engine.message_callback(msg)));
    }

    fn message_callback(&self, msg: &winuser::MSG) {
        let action: Box<dyn Fn(Key)>;
        match msg.message {
            winuser::WM_KEYDOWN => {
                // The 31 lsb == 0 represents the key was up before this WM_KEYDOWN
                let pressed = (msg.lParam & 0x40000000) == 0;
                action = Box::new(move |key| {
                    self.last_key_states.borrow_mut()[key as usize].set_down(true);
                    self.last_key_states.borrow_mut()[key as usize].set_pressed(pressed);
                });
            }
            winuser::WM_KEYUP => {
                action = Box::new(|key| {
                    self.last_key_states.borrow_mut()[key as usize].set_down(false);
                    self.last_key_states.borrow_mut()[key as usize].set_pressed(false);
                });
            }
            _ => return,
        }

        let key = match msg.wParam as i32 {
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
        self.key_states.borrow()[key as usize]
    }
}

impl InputEngineInternal for WindowsInputEngine {
    fn update(&self, delta_sec: f32) {
        self.last_key_states.swap(&self.key_states);
        for state in self.last_key_states.borrow_mut().iter_mut() {
            state.reset_action();
        }
    }

    fn to_input_engine(self: Rc<Self>) -> Rc<dyn InputEngine> {
        self
    }
}
