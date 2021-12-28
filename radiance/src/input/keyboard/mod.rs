#[cfg(target_os = "windows")]
mod windows;
#[cfg(not(target_os = "windows"))]
mod winit;

#[cfg(not(target_os = "windows"))]
pub use self::winit::KeyboardInput;
#[cfg(target_os = "windows")]
pub use windows::KeyboardInput;
