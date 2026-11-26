use std::collections::VecDeque;

use imgui::{Io, MouseButton};

#[cfg(test)]
#[path = "mouse_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug)]
pub enum MouseInput {
    Move([f32; 2]),
    Button {
        button: MouseButton,
        down: bool,
        position: Option<[f32; 2]>,
    },
    Click {
        button: MouseButton,
        position: Option<[f32; 2]>,
    },
    Scroll {
        delta: [f32; 2],
        position: Option<[f32; 2]>,
    },
    Reset,
}

/// App-local UI input. One queued action is applied per rendered UI frame;
/// clicks reserve the following frame for release so no edge is collapsed.
#[derive(Default)]
pub struct SyntheticMouseInput {
    pending: VecDeque<MouseInput>,
    release: Option<MouseButton>,
    position: Option<[f32; 2]>,
    buttons: [bool; 5],
    active: bool,
}

impl SyntheticMouseInput {
    pub fn enqueue(&mut self, input: MouseInput) -> Result<(), &'static str> {
        let (position, delta) = match input {
            MouseInput::Move(position) => (Some(position), None),
            MouseInput::Button { position, .. } | MouseInput::Click { position, .. } => {
                (position, None)
            }
            MouseInput::Scroll { position, delta } => (position, Some(delta)),
            MouseInput::Reset => (None, None),
        };
        if position
            .into_iter()
            .chain(delta)
            .flatten()
            .any(|v| !v.is_finite())
        {
            return Err("mouse coordinates and wheel deltas must be finite");
        }
        if matches!(input, MouseInput::Reset) {
            self.pending.clear();
            self.release = None;
        }
        self.pending.push_back(input);
        Ok(())
    }

    pub(super) fn apply_frame(&mut self, io: &mut Io) {
        let was_active = self.active;
        let mut wheel = None;
        let input = self
            .release
            .take()
            .map(|button| MouseInput::Button {
                button,
                down: false,
                position: None,
            })
            .or_else(|| self.pending.pop_front());
        if let Some(input) = input {
            if !self.active && !matches!(input, MouseInput::Reset) {
                let scale = io.display_framebuffer_scale;
                let position = [io.mouse_pos[0] * scale[0], io.mouse_pos[1] * scale[1]];
                if position.iter().all(|v| v.is_finite() && *v > -f32::MAX) {
                    self.position = Some(position);
                }
                self.active = true;
            }
            match input {
                MouseInput::Move(position) => self.position = Some(position),
                MouseInput::Button {
                    button,
                    down,
                    position,
                } => {
                    if let Some(position) = position {
                        self.position = Some(position);
                    }
                    self.buttons[button as usize] = down;
                }
                MouseInput::Click { button, position } => {
                    if let Some(position) = position {
                        self.position = Some(position);
                    }
                    self.buttons[button as usize] = true;
                    self.release = Some(button);
                }
                MouseInput::Scroll { delta, position } => {
                    if let Some(position) = position {
                        self.position = Some(position);
                    }
                    wheel = Some(delta);
                }
                MouseInput::Reset => {
                    self.position = None;
                    self.buttons = [false; 5];
                    self.active = false;
                }
            }
        }
        if !self.active && !was_active {
            return;
        }
        if let Some(position) = self.position {
            let scale = io.display_framebuffer_scale;
            io.add_mouse_pos_event([position[0] / scale[0], position[1] / scale[1]]);
        } else {
            io.add_mouse_pos_event([-f32::MAX, -f32::MAX]);
        }
        for (index, button) in [
            MouseButton::Left,
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::Extra1,
            MouseButton::Extra2,
        ]
        .into_iter()
        .enumerate()
        {
            io.add_mouse_button_event(button, self.buttons[index]);
        }
        if let Some(wheel) = wheel {
            io.add_mouse_wheel_event(wheel);
        }
    }
}
