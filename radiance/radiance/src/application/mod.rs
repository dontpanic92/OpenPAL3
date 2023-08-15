mod application;
pub mod utils;

#[cfg(vita)]
mod vita;
#[cfg(windows)]
mod windows;
#[cfg(any(linux, macos, android))]
mod winit;

#[cfg(any(linux, macos, android))]
pub use self::winit::Platform;
#[cfg(vita)]
pub use vita::Platform;
#[cfg(windows)]
pub use windows::Platform;

pub use application::Application;
