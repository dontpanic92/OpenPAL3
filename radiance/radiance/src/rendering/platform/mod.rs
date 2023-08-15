#[cfg(vita)]
mod dummy;
#[cfg(windows)]
mod windows;

#[cfg(any(linux, macos, android))]
pub use ::winit::window::Window;
#[cfg(windows)]
pub use windows::Window;

#[cfg(vita)]
pub use dummy::Window;
