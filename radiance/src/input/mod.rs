pub use engine::{InputEngine, Key, KeyState};

pub(crate) use engine::InputEngineInternal;

#[cfg(target_os = "windows")]
pub use windows::WindowsInputEngine;

mod engine;

#[cfg(target_os = "windows")]
mod windows;
