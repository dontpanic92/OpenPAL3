pub(crate) use engine::InputEngineInternal;
pub use engine::{InputEngine, Key, KeyState};

#[cfg(target_os = "windows")]
pub use windows::WindowsInputEngine;

mod engine;

#[cfg(target_os = "windows")]
mod windows;
