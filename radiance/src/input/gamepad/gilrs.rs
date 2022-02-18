use gilrs::{Axis, Button, Event, EventType, Gilrs};

use crate::input::{engine::AxisState, Key, KeyState};

pub struct GilrsInput {
    gilrs: Option<Gilrs>,
}

impl GilrsInput {
    pub fn new() -> Self {
        Self {
            gilrs: Gilrs::new().ok(),
        }
    }

    pub fn process_message(&mut self, states: &mut [KeyState], axis_states: &mut [AxisState]) {
        if let Some(gilrs) = self.gilrs.as_mut() {
            while let Some(Event { id, event, time }) = gilrs.next_event() {
                match event {
                    EventType::ButtonPressed(button, _) => {
                        let key = Self::map_button(button);
                        states[key as usize].set_down(true);
                        states[key as usize].set_pressed(true);
                    }
                    EventType::ButtonReleased(button, _) => {
                        let key = Self::map_button(button);
                        states[key as usize].set_down(false);
                        states[key as usize].set_released(true);
                    }
                    EventType::AxisChanged(axis, value, _) => {
                        let axis = Self::map_axis(axis);
                        axis_states[axis as usize].set_value(value);
                    }
                    _ => {}
                }
            }
        }
    }

    fn map_button(btn: Button) -> Key {
        match btn {
            Button::East => Key::GamePadEast,
            Button::West => Key::GamePadWest,
            Button::South => Key::GamePadSouth,
            Button::North => Key::GamePadNorth,
            Button::DPadUp => Key::GamePadDPadUp,
            Button::DPadDown => Key::GamePadDPadDown,
            Button::DPadLeft => Key::GamePadDPadLeft,
            Button::DPadRight => Key::GamePadDPadRight,
            _ => Key::Unknown,
        }
    }

    fn map_axis(axis: Axis) -> crate::input::Axis {
        match axis {
            Axis::LeftStickX => crate::input::Axis::LeftStickX,
            Axis::LeftStickY => crate::input::Axis::LeftStickY,
            Axis::RightStickX => crate::input::Axis::RightStickX,
            Axis::RightStickY => crate::input::Axis::RightStickY,
            _ => crate::input::Axis::Unknown,
        }
    }
}
