use crate::input::{Key, KeyState};
use winit::event::{DeviceEvent, ElementState, Event, VirtualKeyCode};

pub struct KeyboardInput;

impl KeyboardInput {
    pub fn process_message(&mut self, states: &mut [KeyState], event: &Event<()>) {
        let mut action: Box<dyn FnMut(Key)>;
        let virtual_key;
        match *event {
            Event::DeviceEvent {
                event:
                    DeviceEvent::Key(winit::event::KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(key),
                        ..
                    }),
                ..
            } => {
                virtual_key = key;
                action = Box::new(move |key| {
                    states[key as usize].set_down(true);
                    states[key as usize].set_pressed(true);
                });
            }
            Event::DeviceEvent {
                event:
                    DeviceEvent::Key(winit::event::KeyboardInput {
                        state: ElementState::Released,
                        virtual_keycode: Some(key),
                        ..
                    }),
                ..
            } => {
                virtual_key = key;
                action = Box::new(|key| {
                    states[key as usize].set_down(false);
                    states[key as usize].set_released(true);
                });
            }
            _ => return,
        }

        let key = match virtual_key {
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
            VirtualKeyCode::Up => Key::Up,
            VirtualKeyCode::Down => Key::Down,
            VirtualKeyCode::Left => Key::Left,
            VirtualKeyCode::Right => Key::Right,
            VirtualKeyCode::Space => Key::Space,
            _ => return,
        };

        action(key);
    }
}
