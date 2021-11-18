pub(crate) use engine::InputEngineInternal;
pub use engine::{Axis, InputEngine, Key, KeyState};

#[cfg(target_os = "windows")]
pub use windows::WindowsInputEngine;

mod engine;
mod gamepad;
mod keyboard;

#[cfg(target_os = "windows")]
mod windows;
