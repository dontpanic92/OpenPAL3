use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::input::{InputEngine, Key};

use crate::comdef::services::{IInputService, IInputServiceImpl};

pub struct InputService {
    input: Rc<RefCell<dyn InputEngine>>,
}

ComObject_InputService!(super::InputService);

impl InputService {
    pub fn create(input: Rc<RefCell<dyn InputEngine>>) -> ComRc<IInputService> {
        ComRc::from_object(Self { input })
    }
}

impl IInputServiceImpl for InputService {
    fn key_down(&self, keycode: i32) -> bool {
        self.input
            .borrow()
            .get_key_state(key_from_i32(keycode))
            .is_down()
    }

    fn key_pressed(&self, keycode: i32) -> bool {
        self.input
            .borrow()
            .get_key_state(key_from_i32(keycode))
            .pressed()
    }

    fn mouse_x(&self) -> f32 {
        // The current InputEngine trait exposes keys and axes but no mouse position.
        0.0
    }

    fn mouse_y(&self) -> f32 {
        // The current InputEngine trait exposes keys and axes but no mouse position.
        0.0
    }
}

fn key_from_i32(keycode: i32) -> Key {
    match keycode {
        0 => Key::Space,
        1 => Key::A,
        2 => Key::B,
        3 => Key::C,
        4 => Key::D,
        5 => Key::E,
        6 => Key::F,
        7 => Key::G,
        8 => Key::H,
        9 => Key::I,
        10 => Key::J,
        11 => Key::K,
        12 => Key::L,
        13 => Key::M,
        14 => Key::N,
        15 => Key::O,
        16 => Key::P,
        17 => Key::Q,
        18 => Key::R,
        19 => Key::S,
        20 => Key::T,
        21 => Key::U,
        22 => Key::V,
        23 => Key::W,
        24 => Key::X,
        25 => Key::Y,
        26 => Key::Z,
        27 => Key::Num1,
        28 => Key::Num2,
        29 => Key::Num3,
        30 => Key::Num4,
        31 => Key::Num5,
        32 => Key::Num6,
        33 => Key::Num7,
        34 => Key::Num8,
        35 => Key::Num9,
        36 => Key::Num0,
        37 => Key::Tilde,
        38 => Key::Escape,
        39 => Key::Left,
        40 => Key::Up,
        41 => Key::Right,
        42 => Key::Down,
        43 => Key::GamePadEast,
        44 => Key::GamePadSouth,
        45 => Key::GamePadWest,
        46 => Key::GamePadNorth,
        47 => Key::GamePadDPadUp,
        48 => Key::GamePadDPadDown,
        49 => Key::GamePadDPadLeft,
        50 => Key::GamePadDPadRight,
        _ => Key::Unknown,
    }
}
