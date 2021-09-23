pub(crate) use engine::InputEngineInternal;
pub use engine::{InputEngine, Key, Axis, KeyState};

#[cfg(target_os = "windows")]
pub use windows::WindowsInputEngine;

mod engine;
mod keyboard;
mod gamepad;

#[cfg(target_os = "windows")]
mod windows;
