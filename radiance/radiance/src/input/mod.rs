pub use engine::CoreInputEngine;
pub use synthetic::SyntheticInputBridge;

mod engine;
mod gamepad;
mod keyboard;
mod mouse;
mod synthetic;

use std::{cell::RefCell, rc::Rc};

pub trait InputEngine {
    fn get_key_state(&self, key: Key) -> KeyState;
    fn get_axis_state(&self, axis: Axis) -> AxisState;

    /// State of a mouse button (`Left`, `Right`, `Middle`). Defaults to
    /// "up" on platforms that don't surface mouse events.
    fn get_mouse_button_state(&self, _button: MouseButton) -> KeyState {
        KeyState::new(false, false, false)
    }

    /// Cursor motion accumulated since the previous engine `update()`,
    /// in raw screen pixels. `(0.0, 0.0)` when the platform doesn't
    /// expose mouse motion.
    fn get_mouse_delta(&self) -> (f32, f32) {
        (0.0, 0.0)
    }

    /// Mouse wheel ticks accumulated since the previous engine
    /// `update()`. Normalized so one detent of a typical wheel reports
    /// `1.0` (positive = scroll up / away from the user).
    fn get_mouse_wheel(&self) -> f32 {
        0.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum MouseButton {
    Left = 0,
    Right,
    Middle,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Axis {
    LeftStickX = 0,
    LeftStickY,
    RightStickX,
    RightStickY,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Key {
    Space = 0,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    Num0,
    Tilde,
    Escape,
    Left,
    Up,
    Right,
    Down,
    GamePadEast,
    GamePadSouth,
    GamePadWest,
    GamePadNorth,
    GamePadDPadUp,
    GamePadDPadDown,
    GamePadDPadLeft,
    GamePadDPadRight,
    Unknown,
}

impl Key {
    /// Case-insensitive parse of a [`Key`] from its Rust identifier
    /// (e.g. `"F"`, `"space"`, `"GamePadEast"`). Returns `None` (not
    /// `Key::Unknown`) for unrecognized names so callers can surface a
    /// real error.
    pub fn from_name(name: &str) -> Option<Self> {
        let n = name.trim();
        if n.is_empty() {
            return None;
        }
        Some(match n.to_ascii_lowercase().as_str() {
            "space" => Self::Space,
            "a" => Self::A,
            "b" => Self::B,
            "c" => Self::C,
            "d" => Self::D,
            "e" => Self::E,
            "f" => Self::F,
            "g" => Self::G,
            "h" => Self::H,
            "i" => Self::I,
            "j" => Self::J,
            "k" => Self::K,
            "l" => Self::L,
            "m" => Self::M,
            "n" => Self::N,
            "o" => Self::O,
            "p" => Self::P,
            "q" => Self::Q,
            "r" => Self::R,
            "s" => Self::S,
            "t" => Self::T,
            "u" => Self::U,
            "v" => Self::V,
            "w" => Self::W,
            "x" => Self::X,
            "y" => Self::Y,
            "z" => Self::Z,
            "num1" | "1" => Self::Num1,
            "num2" | "2" => Self::Num2,
            "num3" | "3" => Self::Num3,
            "num4" | "4" => Self::Num4,
            "num5" | "5" => Self::Num5,
            "num6" | "6" => Self::Num6,
            "num7" | "7" => Self::Num7,
            "num8" | "8" => Self::Num8,
            "num9" | "9" => Self::Num9,
            "num0" | "0" => Self::Num0,
            "tilde" | "~" | "`" => Self::Tilde,
            "escape" | "esc" => Self::Escape,
            "left" => Self::Left,
            "up" => Self::Up,
            "right" => Self::Right,
            "down" => Self::Down,
            "gamepadeast" => Self::GamePadEast,
            "gamepadsouth" => Self::GamePadSouth,
            "gamepadwest" => Self::GamePadWest,
            "gamepadnorth" => Self::GamePadNorth,
            "gamepaddpadup" => Self::GamePadDPadUp,
            "gamepaddpaddown" => Self::GamePadDPadDown,
            "gamepaddpadleft" => Self::GamePadDPadLeft,
            "gamepaddpadright" => Self::GamePadDPadRight,
            _ => return None,
        })
    }
}

impl Axis {
    /// Case-insensitive parse of an [`Axis`] from its Rust identifier.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name.trim().to_ascii_lowercase().as_str() {
            "leftstickx" => Self::LeftStickX,
            "leftsticky" => Self::LeftStickY,
            "rightstickx" => Self::RightStickX,
            "rightsticky" => Self::RightStickY,
            _ => return None,
        })
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AxisState {
    value: f32,
}

impl AxisState {
    pub(crate) fn new() -> Self {
        Self { value: 0. }
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub(crate) fn set_value(&mut self, value: f32) {
        self.value = value;
    }
}

#[derive(Copy, Clone, Debug)]
pub struct KeyState {
    is_down: bool,
    pressed: bool,
    released: bool,
}

impl KeyState {
    pub(crate) fn new(is_down: bool, pressed: bool, released: bool) -> KeyState {
        KeyState {
            is_down,
            pressed,
            released,
        }
    }

    pub fn is_down(&self) -> bool {
        self.is_down
    }

    pub fn is_up(&self) -> bool {
        !self.is_down
    }

    pub fn pressed(&self) -> bool {
        self.pressed
    }

    pub fn released(&self) -> bool {
        self.released
    }

    pub(crate) fn reset_action(&mut self) {
        self.pressed = false;
        self.released = false;
    }

    pub(crate) fn set_down(&mut self, down: bool) {
        self.is_down = down;
    }

    pub(crate) fn set_pressed(&mut self, pressed: bool) {
        self.pressed = pressed;
    }

    pub(crate) fn set_released(&mut self, released: bool) {
        self.released = released;
    }
}

pub(crate) trait InputEngineInternal: InputEngine {
    fn update(&mut self, delta_sec: f32);
    fn as_input_engine(&self) -> Rc<RefCell<dyn InputEngine>>;
}
