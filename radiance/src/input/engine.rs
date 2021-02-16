use std::{cell::RefCell, rc::Rc};

pub trait InputEngine {
    fn get_key_state(&self, key: Key) -> KeyState;
}

#[derive(Clone, Copy, PartialEq, Debug)]
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
    Left,
    Up,
    Right,
    Down,
    Unknown,
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
