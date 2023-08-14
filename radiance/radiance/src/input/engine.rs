use std::{
    cell::RefCell,
    rc::{Rc, Weak}, mem::swap,
};

#[cfg(windows)]
use winapi::um::winuser;

#[cfg(any(linux, macos, android))]
use winit::event::Event;

use crate::application::Platform;

use super::{keyboard::KeyboardInput, gamepad::GamepadInput, Axis, AxisState, Key, KeyState, InputEngine, InputEngineInternal};

pub struct CoreInputEngine {
    input_engine: Weak<RefCell<CoreInputEngine>>,
    last_key_states: Box<Vec<KeyState>>,
    key_states: Box<Vec<KeyState>>,
    axis_states: Box<Vec<AxisState>>,

    keyboard: KeyboardInput,
    gamepad: GamepadInput,
}

impl CoreInputEngine {
    pub fn new(platform: &mut Platform) -> Rc<RefCell<CoreInputEngine>> {
        let engine = Rc::new(RefCell::new(CoreInputEngine {
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
            keyboard: KeyboardInput,
            gamepad: GamepadInput::new(),
        }));

        engine.borrow_mut().input_engine = Rc::downgrade(&engine);
        Self::append_message_callback_to(engine.clone(), platform);
        engine
    }

    fn append_message_callback_to(_self: Rc<RefCell<Self>>, platform: &mut Platform) {
        #[cfg(any(windows, linux, macos, android))]
        platform.add_message_callback(Box::new(move |msg| {
            _self.borrow_mut().message_callback(msg)
        }));
    }

    #[cfg(windows)]
    fn message_callback(&mut self, msg: &winuser::MSG) {
        self.keyboard
            .process_message(&mut self.last_key_states, msg);
    }

    #[cfg(any(linux, macos, android))]
    fn message_callback(&mut self, msg: &Event<()>) {
        self.keyboard
            .process_message(&mut self.last_key_states, msg);
    }
}

impl InputEngine for CoreInputEngine {
    fn get_key_state(&self, key: Key) -> KeyState {
        self.key_states[key as usize]
    }

    fn get_axis_state(&self, axis: Axis) -> AxisState {
        self.axis_states[axis as usize]
    }
}

impl InputEngineInternal for CoreInputEngine {
    fn update(&mut self, delta_sec: f32) {
        self.gamepad
            .process_message(&mut self.last_key_states, &mut self.axis_states);

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
