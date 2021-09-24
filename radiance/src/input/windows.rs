use super::{
    engine::{InputEngine, InputEngineInternal, Key, KeyState},
    gamepad::GilrsInput,
    keyboard::WindowsKeyboardInput,
};
use crate::{application::Platform, input::engine::{Axis, AxisState}};
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
    axis_states: Box<Vec<AxisState>>,

    keyboard: WindowsKeyboardInput,
    gamepad: GilrsInput,
}

impl WindowsInputEngine {
    pub fn new(platform: &mut Platform) -> Rc<RefCell<WindowsInputEngine>> {
        let engine = Rc::new(RefCell::new(WindowsInputEngine {
            input_engine: Weak::new(),
            last_key_states: Box::new(vec![
                KeyState::new(false, false, false);
                Key::Unknown as usize + 1
            ]),
            key_states: Box::new(vec![
                KeyState::new(false, false, false);
                Key::Unknown as usize + 1
            ]),
            axis_states: Box::new(vec![AxisState::new(); Axis::Unknown as usize + 1]),
            keyboard: WindowsKeyboardInput,
            gamepad: GilrsInput::new(),
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
        self.keyboard
            .process_message(&mut self.last_key_states, msg);
    }
}

impl InputEngine for WindowsInputEngine {
    fn get_key_state(&self, key: Key) -> KeyState {
        self.key_states[key as usize]
    }

    fn get_axis_state(&self, axis: Axis) -> AxisState {
        self.axis_states[axis as usize]
    }
}

impl InputEngineInternal for WindowsInputEngine {
    fn update(&mut self, delta_sec: f32) {
        self.gamepad.process_message(&mut self.last_key_states, &mut self.axis_states);

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
