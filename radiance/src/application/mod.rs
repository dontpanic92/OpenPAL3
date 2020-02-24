mod application;
pub mod utils;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
use windows::Platform;

pub use application::{Application, ApplicationCallbacks, DefaultApplication};
