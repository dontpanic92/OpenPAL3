mod application;
pub mod utils;

#[cfg(windows)]
mod windows;
#[cfg(any(linux, macos, android))]
mod winit;
#[cfg(vita)]
mod vita;

#[cfg(windows)]
pub use windows::Platform;
#[cfg(any(linux, macos, android))]
pub use self::winit::Platform;
#[cfg(vita)]
pub use vita::Platform;


pub use application::Application;
