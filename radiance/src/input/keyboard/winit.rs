use crate::input::{Key, KeyState};
#[cfg(any(target_os = "macos", target_os = "android"))]
use winit::event::WindowEvent;
use winit::event::{DeviceEvent, ElementState, Event, VirtualKeyCode};

pub struct KeyboardInput;

impl KeyboardInput {
    pub fn process_message(&mut self, states: &mut [KeyState], event: &Event<()>) {
        let mut action: Box<dyn FnMut(Key)>;
        let virtual_code: Option<VirtualKeyCode>;
        let scan_code: u32;
        match *event {
            Event::DeviceEvent {
                event:
                    DeviceEvent::Key(winit::event::KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode,
                        scancode,
                        ..
                    }),
                ..
            } => {
                virtual_code = virtual_keycode;
                scan_code = scancode;
                action = Box::new(move |key| {
                    states[key as usize].set_down(true);
                    states[key as usize].set_pressed(true);
                });
            }
            Event::DeviceEvent {
                event:
                    DeviceEvent::Key(winit::event::KeyboardInput {
                        state: ElementState::Released,
                        virtual_keycode,
                        scancode,
                        ..
                    }),
                ..
            } => {
                virtual_code = virtual_keycode;
                scan_code = scancode;
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
                        input:
                            winit::event::KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode,
                                scancode,
                                ..
                            },
                        ..
                    },
                ..
            } => {
                virtual_code = virtual_keycode;
                scan_code = scancode;
                action = Box::new(move |key| {
                    states[key as usize].set_down(true);
                    states[key as usize].set_pressed(true);
                });
            }
            #[cfg(any(target_os = "macos", target_os = "android"))]
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            winit::event::KeyboardInput {
                                state: ElementState::Released,
                                virtual_keycode,
                                scancode,
                                ..
                            },
                        ..
                    },
                ..
            } => {
                virtual_code = virtual_keycode;
                scan_code = scancode;
                action = Box::new(|key| {
                    states[key as usize].set_down(false);
                    states[key as usize].set_released(true);
                });
            }
            _ => return,
        }

        let key = if virtual_code.is_none() {
            Key::Unknown
        } else {
            match virtual_code.unwrap() {
                VirtualKeyCode::Key0 => Key::Num0,
                VirtualKeyCode::Key1 => Key::Num1,
                VirtualKeyCode::Key2 => Key::Num2,
                VirtualKeyCode::Key3 => Key::Num3,
                VirtualKeyCode::Key4 => Key::Num4,
                VirtualKeyCode::Key5 => Key::Num5,
                VirtualKeyCode::Key6 => Key::Num6,
                VirtualKeyCode::Key7 => Key::Num7,
                VirtualKeyCode::Key8 => Key::Num8,
                VirtualKeyCode::Key9 => Key::Num9,
                VirtualKeyCode::A => Key::A,
                VirtualKeyCode::B => Key::B,
                VirtualKeyCode::C => Key::C,
                VirtualKeyCode::D => Key::D,
                VirtualKeyCode::E => Key::E,
                VirtualKeyCode::F => Key::F,
                VirtualKeyCode::G => Key::G,
                VirtualKeyCode::H => Key::H,
                VirtualKeyCode::I => Key::I,
                VirtualKeyCode::J => Key::J,
                VirtualKeyCode::K => Key::K,
                VirtualKeyCode::L => Key::L,
                VirtualKeyCode::M => Key::M,
                VirtualKeyCode::N => Key::N,
                VirtualKeyCode::O => Key::O,
                VirtualKeyCode::P => Key::P,
                VirtualKeyCode::Q => Key::Q,
                VirtualKeyCode::R => Key::R,
                VirtualKeyCode::S => Key::S,
                VirtualKeyCode::T => Key::T,
                VirtualKeyCode::U => Key::U,
                VirtualKeyCode::V => Key::V,
                VirtualKeyCode::W => Key::W,
                VirtualKeyCode::X => Key::X,
                VirtualKeyCode::Y => Key::Y,
                VirtualKeyCode::Z => Key::Z,
                VirtualKeyCode::Grave => Key::Tilde,
                VirtualKeyCode::Escape => Key::Escape,
                VirtualKeyCode::Up => Key::Up,
                VirtualKeyCode::Down => Key::Down,
                VirtualKeyCode::Left => Key::Left,
                VirtualKeyCode::Right => Key::Right,
                VirtualKeyCode::Space => Key::Space,
                _ => Key::Unknown,
            }
        };

        let key = if key != Key::Unknown {
            key
        } else {
            match scan_code {
                11 => Key::Num0,
                2 => Key::Num1,
                3 => Key::Num2,
                4 => Key::Num3,
                5 => Key::Num4,
                6 => Key::Num5,
                7 => Key::Num6,
                8 => Key::Num7,
                9 => Key::Num8,
                10 => Key::Num9,
                30 => Key::A,
                48 => Key::B,
                46 => Key::C,
                32 => Key::D,
                18 => Key::E,
                33 => Key::F,
                34 => Key::G,
                35 => Key::H,
                23 => Key::I,
                36 => Key::J,
                37 => Key::K,
                38 => Key::L,
                50 => Key::M,
                49 => Key::N,
                24 => Key::O,
                25 => Key::P,
                16 => Key::Q,
                19 => Key::R,
                31 => Key::S,
                20 => Key::T,
                22 => Key::U,
                47 => Key::V,
                17 => Key::W,
                45 => Key::X,
                21 => Key::Y,
                44 => Key::Z,
                41 => Key::Tilde,
                1 => Key::Escape,
                103 => Key::Up,
                108 => Key::Down,
                105 => Key::Left,
                106 => Key::Right,
                57 => Key::Space,
                _ => Key::Unknown,
            }
        };
        action(key);
    }
}
