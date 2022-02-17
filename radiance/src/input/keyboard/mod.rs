#[cfg(target_os = "windows")]
mod windows;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "android",))]
mod winit;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "android",))]
pub use self::winit::KeyboardInput;
#[cfg(target_os = "windows")]
pub use windows::KeyboardInput;

mod null;

#[cfg(target_os = "psp")]
pub use null::KeyboardInput;
