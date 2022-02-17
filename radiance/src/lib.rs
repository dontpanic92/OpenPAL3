#![feature(arbitrary_self_types)]
#![feature(bool_to_option)]
#![allow(unused_variables)]
#![cfg_attr(target_os = "psp", no_std)]

#[macro_use]
mod macros;

pub mod application;
pub mod audio;

#[cfg_attr(
    any(
        target_os = "windows",
        target_os = "linux",
        target_os = "macos",
        target_os = "android",
    ),
    path = "imgui/mod.rs"
)]
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "android",
))]
pub mod ui;

#[cfg(target_os = "psp")]
pub mod ui;

pub mod input;
pub mod math;
pub mod radiance;
pub mod rendering;
pub mod scene;
pub mod video;

mod constants;

extern crate alloc;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate downcast_rs;

#[macro_use]
extern crate bitflags;

#[cfg(all(feature = "std", feature = "no_std"))]
compile_error!("feature \"std\" and feature \"no_std\" cannot be enabled at the same time");

#[cfg(not(any(feature = "std", feature = "no_std")))]
compile_error!("One of feature \"std\" and feature \"no_std\" must be enabled");
