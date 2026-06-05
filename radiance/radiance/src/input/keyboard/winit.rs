use crate::input::{Key, KeyState};
use winit::event::{DeviceEvent, ElementState, KeyEvent, RawKeyEvent, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct KeyboardInput;

impl KeyboardInput {
    /// Apply a winit `DeviceEvent` (keyboard arm) to the key-state
    /// buffer. Used on every desktop platform — device events fire even
    /// when our window doesn't have focus.
    pub fn process_device_event(&mut self, states: &mut [KeyState], event: &DeviceEvent) {
        let DeviceEvent::Key(raw) = event else {
            return;
        };
        apply_raw_key(states, raw);
    }

    /// Apply a winit `WindowEvent` (KeyboardInput arm). winit only
    /// delivers keyboard input through `WindowEvent` on macOS and
    /// Android — on other desktop targets the equivalent comes via
    /// `DeviceEvent::Key`.
    #[cfg(any(target_os = "macos", target_os = "android"))]
    pub fn process_window_event(&mut self, states: &mut [KeyState], event: &WindowEvent) {
        let WindowEvent::KeyboardInput { event, .. } = event else {
            return;
        };
        let KeyEvent {
            state,
            physical_key: PhysicalKey::Code(code),
            ..
        } = event
        else {
            return;
        };
        apply_key(states, *state, *code);
    }

    #[cfg(not(any(target_os = "macos", target_os = "android")))]
    pub fn process_window_event(&mut self, _states: &mut [KeyState], _event: &WindowEvent) {}
}

fn apply_raw_key(states: &mut [KeyState], raw: &RawKeyEvent) {
    let PhysicalKey::Code(code) = raw.physical_key else {
        return;
    };
    apply_key(states, raw.state, code);
}

fn apply_key(states: &mut [KeyState], state: ElementState, code: KeyCode) {
    let key = map_keycode(code);
    match state {
        ElementState::Pressed => {
            states[key as usize].set_down(true);
            states[key as usize].set_pressed(true);
        }
        ElementState::Released => {
            states[key as usize].set_down(false);
            states[key as usize].set_released(true);
        }
    }
}

fn map_keycode(code: KeyCode) -> Key {
    match code {
        KeyCode::Digit0 => Key::Num0,
        KeyCode::Digit1 => Key::Num1,
        KeyCode::Digit2 => Key::Num2,
        KeyCode::Digit3 => Key::Num3,
        KeyCode::Digit4 => Key::Num4,
        KeyCode::Digit5 => Key::Num5,
        KeyCode::Digit6 => Key::Num6,
        KeyCode::Digit7 => Key::Num7,
        KeyCode::Digit8 => Key::Num8,
        KeyCode::Digit9 => Key::Num9,
        KeyCode::KeyA => Key::A,
        KeyCode::KeyB => Key::B,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyD => Key::D,
        KeyCode::KeyE => Key::E,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyG => Key::G,
        KeyCode::KeyH => Key::H,
        KeyCode::KeyI => Key::I,
        KeyCode::KeyJ => Key::J,
        KeyCode::KeyK => Key::K,
        KeyCode::KeyL => Key::L,
        KeyCode::KeyM => Key::M,
        KeyCode::KeyN => Key::N,
        KeyCode::KeyO => Key::O,
        KeyCode::KeyP => Key::P,
        KeyCode::KeyQ => Key::Q,
        KeyCode::KeyR => Key::R,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyT => Key::T,
        KeyCode::KeyU => Key::U,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyW => Key::W,
        KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y,
        KeyCode::KeyZ => Key::Z,
        KeyCode::Backquote => Key::Tilde,
        KeyCode::Escape => Key::Escape,
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::ArrowLeft => Key::Left,
        KeyCode::ArrowRight => Key::Right,
        KeyCode::Space => Key::Space,
        _ => Key::Unknown,
    }
}
