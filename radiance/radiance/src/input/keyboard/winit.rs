use crate::input::{Key, KeyState};
#[cfg(any(target_os = "macos", target_os = "android"))]
use winit::event::WindowEvent;
use winit::{
    event::{DeviceEvent, ElementState, Event, KeyEvent, RawKeyEvent},
    keyboard::{KeyCode, PhysicalKey},
};

pub struct KeyboardInput;

impl KeyboardInput {
    pub fn process_message(&mut self, states: &mut [KeyState], event: &Event<()>) {
        let mut action: Box<dyn FnMut(Key)>;
        let keycode: Option<KeyCode>;
        match *event {
            Event::DeviceEvent {
                event:
                    DeviceEvent::Key(RawKeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(code),
                    }),
                ..
            } => {
                keycode = Some(code);
                action = Box::new(move |key| {
                    states[key as usize].set_down(true);
                    states[key as usize].set_pressed(true);
                });
            }
            Event::DeviceEvent {
                event:
                    DeviceEvent::Key(RawKeyEvent {
                        state: ElementState::Released,
                        physical_key: PhysicalKey::Code(code),
                    }),
                ..
            } => {
                keycode = Some(code);
                action = Box::new(|key| {
                    states[key as usize].set_down(false);
                    states[key as usize].set_released(true);
                });
            }
            // on macOS / Android keyboard input events are only in WindowEvent
            #[cfg(any(target_os = "macos", target_os = "android"))]
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        event:
                            winit::event::KeyEvent {
                                state: ElementState::Pressed,
                                physical_key: PhysicalKey::Code(code),
                                ..
                            },
                        ..
                    },
                ..
            } => {
                keycode = Some(code);
                action = Box::new(move |key| {
                    states[key as usize].set_down(true);
                    states[key as usize].set_pressed(true);
                });
            }
            #[cfg(any(target_os = "macos", target_os = "android"))]
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        event:
                            winit::event::KeyEvent {
                                state: ElementState::Released,
                                physical_key: PhysicalKey::Code(code),
                                ..
                            },
                        ..
                    },
                ..
            } => {
                keycode = Some(code);
                action = Box::new(|key| {
                    states[key as usize].set_down(false);
                    states[key as usize].set_released(true);
                });
            }
            _ => return,
        }

        let key = if keycode.is_none() {
            Key::Unknown
        } else {
            match keycode.unwrap() {
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
        };

        action(key);
    }
}
