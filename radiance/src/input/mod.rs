pub(crate) use engine::InputEngineInternal;
pub use engine::{Axis, InputEngine, Key, KeyState};

pub use generic::GenericInputEngine;
mod engine;
mod gamepad;
mod keyboard;

mod generic;
