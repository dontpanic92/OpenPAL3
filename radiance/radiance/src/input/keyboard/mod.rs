#[cfg(windows)]
mod windows;
#[cfg(any(linux, macos, android))]
mod winit;
#[cfg(vita)]
mod nop;

#[cfg(windows)]
pub use windows::KeyboardInput;
#[cfg(any(linux, macos, android))]
pub use self::winit::KeyboardInput;
#[cfg(vita)]
pub use nop::KeyboardInput;
