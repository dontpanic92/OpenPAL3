#[cfg(windows)]
mod windows;
#[cfg(vita)]
mod dummy;

#[cfg(windows)]
pub use windows::Window;
#[cfg(any(linux, macos, android))]
pub use ::winit::window::Window;

#[cfg(vita)]
pub use dummy::Window;
