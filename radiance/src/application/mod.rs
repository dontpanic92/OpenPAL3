mod application;
pub mod utils;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::Platform;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
mod winit;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
pub use self::winit::Platform;

#[cfg(target_os = "psp")]
mod psp;
#[cfg(target_os = "psp")]
pub use psp::Platform;

pub use application::{Application, ApplicationExtension, DefaultApplication};
