mod application;
pub mod utils;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub use windows::Platform;

pub use application::{Application, ApplicationExtension, DefaultApplication};
