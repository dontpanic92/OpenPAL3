#[cfg(any(windows, linux, macos, android))]
mod gilrs;
#[cfg(vita)]
mod vita;

#[cfg(any(windows, linux, macos, android))]
pub use self::gilrs::GilrsInput as GamepadInput;
#[cfg(vita)]
pub use vita::VitaGamepadInput as GamepadInput;
