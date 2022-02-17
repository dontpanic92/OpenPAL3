#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "android",
))]
mod gilrs;
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "android",
))]
pub type GamePadInput = gilrs::GilrsInput;

#[cfg(target_os = "psp")]
mod psp;

#[cfg(target_os = "psp")]
pub type GamePadInput = psp::PspInput;
