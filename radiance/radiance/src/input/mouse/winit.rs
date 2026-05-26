use crate::input::{KeyState, MouseButton};
use winit::event::{
    DeviceEvent, ElementState, Event, MouseButton as WinitMouseButton, MouseScrollDelta,
    WindowEvent,
};

// Approximate pixel-equivalent for one wheel "line" so script-side
// wheel deltas are comparable across LineDelta / PixelDelta backends.
const LINE_PIXEL_HEIGHT: f32 = 16.0;

pub struct MouseInput;

impl MouseInput {
    pub fn new() -> Self {
        Self
    }

    pub fn process_message(
        &mut self,
        button_states: &mut [KeyState],
        delta: &mut (f32, f32),
        wheel: &mut f32,
        event: &Event<()>,
    ) {
        match event {
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta: (dx, dy) },
                ..
            } => {
                delta.0 += *dx as f32;
                delta.1 += *dy as f32;
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button, .. },
                ..
            } => {
                let mapped = match button {
                    WinitMouseButton::Left => MouseButton::Left,
                    WinitMouseButton::Right => MouseButton::Right,
                    WinitMouseButton::Middle => MouseButton::Middle,
                    _ => MouseButton::Unknown,
                };
                if matches!(mapped, MouseButton::Unknown) {
                    return;
                }
                let down = matches!(state, ElementState::Pressed);
                set_button(button_states, mapped, down);
            }
            Event::WindowEvent {
                event: WindowEvent::MouseWheel { delta: scroll, .. },
                ..
            } => {
                let ticks = match scroll {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    // Convert raw pixels into wheel-detent-equivalents so
                    // the script can treat one notch ~= 1.0 either way.
                    MouseScrollDelta::PixelDelta(p) => (p.y as f32) / LINE_PIXEL_HEIGHT,
                };
                *wheel += ticks;
            }
            _ => {}
        }
    }
}

fn set_button(states: &mut [KeyState], button: MouseButton, down: bool) {
    let idx = button as usize;
    if down {
        states[idx].set_down(true);
        states[idx].set_pressed(true);
    } else {
        states[idx].set_down(false);
        states[idx].set_released(true);
    }
}
