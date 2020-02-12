mod application;
mod extensions;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
use windows::Platform;

pub use application::{Application, DefaultApplication, ApplicationCallbacks};
