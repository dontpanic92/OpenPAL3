use std::{
    cell::RefCell,
    mem::swap,
    rc::{Rc, Weak},
};

#[cfg(windows)]
use winapi::um::winuser;

use crate::application::Platform;

use super::{
    Axis, AxisState, InputEngine, InputEngineInternal, Key, KeyState, MouseButton,
    gamepad::GamepadInput, keyboard::KeyboardInput, mouse::MouseInput,
};

pub struct CoreInputEngine {
    input_engine: Weak<RefCell<CoreInputEngine>>,
    last_key_states: Box<Vec<KeyState>>,
    key_states: Box<Vec<KeyState>>,
    axis_states: Box<Vec<AxisState>>,

    // Per-frame mouse state. `last_*` are written by the platform
    // message callback; the public `*_states`/`*_delta`/`*_wheel`
    // values are flipped in via `update()` so the script sees a
    // stable snapshot for the whole frame.
    last_mouse_button_states: Box<Vec<KeyState>>,
    mouse_button_states: Box<Vec<KeyState>>,
    last_mouse_delta: (f32, f32),
    mouse_delta: (f32, f32),
    last_mouse_wheel: f32,
    mouse_wheel: f32,

    keyboard: KeyboardInput,
    gamepad: GamepadInput,
    mouse: MouseInput,
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
            last_mouse_button_states: Box::new(vec![
                KeyState::new(false, false, false);
                MouseButton::Unknown as usize + 1
            ]),
            mouse_button_states: Box::new(vec![
                KeyState::new(false, false, false);
                MouseButton::Unknown as usize + 1
            ]),
            last_mouse_delta: (0.0, 0.0),
            mouse_delta: (0.0, 0.0),
            last_mouse_wheel: 0.0,
            mouse_wheel: 0.0,
            keyboard: KeyboardInput,
            gamepad: GamepadInput::new(),
            mouse: MouseInput::new(),
        }));

        engine.borrow_mut().input_engine = Rc::downgrade(&engine);
        Self::append_message_callback_to(engine.clone(), platform);
        engine
    }

    fn append_message_callback_to(_self: Rc<RefCell<Self>>, platform: &mut Platform) {
        #[cfg(windows)]
        platform.add_message_callback(Box::new(move |msg| {
            _self.borrow_mut().message_callback(msg)
        }));

        #[cfg(any(linux, macos, android))]
        {
            let window_self = _self.clone();
            platform.add_window_event_callback(Box::new(move |_window_id, event| {
                let mut me = window_self.borrow_mut();
                let CoreInputEngine {
                    last_key_states,
                    last_mouse_button_states,
                    last_mouse_wheel,
                    keyboard,
                    mouse,
                    ..
                } = &mut *me;
                keyboard.process_window_event(last_key_states, event);
                mouse.process_window_event(last_mouse_button_states, last_mouse_wheel, event);
            }));

            let device_self = _self;
            platform.add_device_event_callback(Box::new(move |_device_id, event| {
                let mut me = device_self.borrow_mut();
                let CoreInputEngine {
                    last_key_states,
                    last_mouse_delta,
                    keyboard,
                    mouse,
                    ..
                } = &mut *me;
                keyboard.process_device_event(last_key_states, event);
                mouse.process_device_event(last_mouse_delta, event);
            }));
        }
    }

    #[cfg(windows)]
    fn message_callback(&mut self, msg: &winuser::MSG) {
        self.keyboard
            .process_message(&mut self.last_key_states, msg);
        self.mouse.process_message(
            &mut self.last_mouse_button_states,
            &mut self.last_mouse_delta,
            &mut self.last_mouse_wheel,
            msg,
        );
    }
}

impl InputEngine for CoreInputEngine {
    fn get_key_state(&self, key: Key) -> KeyState {
        self.key_states[key as usize]
    }

    fn get_axis_state(&self, axis: Axis) -> AxisState {
        self.axis_states[axis as usize]
    }

    fn get_mouse_button_state(&self, button: MouseButton) -> KeyState {
        self.mouse_button_states[button as usize]
    }

    fn get_mouse_delta(&self) -> (f32, f32) {
        self.mouse_delta
    }

    fn get_mouse_wheel(&self) -> f32 {
        self.mouse_wheel
    }
}

impl InputEngineInternal for CoreInputEngine {
    fn update(&mut self, _delta_sec: f32) {
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

        // Flip mouse button states the same way: callers see this
        // frame's pressed/released, and the next-frame buffer carries
        // forward only the held-down flags.
        swap(
            &mut self.mouse_button_states,
            &mut self.last_mouse_button_states,
        );
        for (next_state, cur_state) in self
            .last_mouse_button_states
            .iter_mut()
            .zip(self.mouse_button_states.iter_mut())
        {
            next_state.reset_action();
            next_state.set_down(cur_state.is_down());
        }

        // Mouse motion / wheel are pure per-frame deltas: publish what
        // accumulated since the previous update and reset the writer.
        self.mouse_delta = self.last_mouse_delta;
        self.last_mouse_delta = (0.0, 0.0);
        self.mouse_wheel = self.last_mouse_wheel;
        self.last_mouse_wheel = 0.0;
    }

    fn as_input_engine(&self) -> Rc<RefCell<dyn InputEngine>> {
        self.input_engine.upgrade().unwrap()
    }
}
