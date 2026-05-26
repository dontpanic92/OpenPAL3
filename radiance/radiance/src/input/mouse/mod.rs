#[cfg(vita)]
mod nop;
#[cfg(windows)]
mod windows;
#[cfg(any(linux, macos, android))]
mod winit;

#[cfg(any(linux, macos, android))]
pub use self::winit::MouseInput;
#[cfg(vita)]
pub use nop::MouseInput;
#[cfg(windows)]
pub use windows::MouseInput;
