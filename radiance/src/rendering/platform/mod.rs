#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::Window;

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "android",
))]
mod winit;
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "android",
))]
pub use ::winit::window::Window;

#[cfg(target_os = "psp")]
mod psp;
#[cfg(target_os = "psp")]
pub use psp::Window;
