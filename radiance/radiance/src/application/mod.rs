mod application;
pub mod utils;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(not(target_os = "windows"))]
mod winit;

#[cfg(not(target_os = "windows"))]
pub use self::winit::Platform;
#[cfg(target_os = "windows")]
pub use windows::Platform;

pub use application::Application;
