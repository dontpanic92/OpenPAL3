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
    path = "imgui"
)]
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

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate bitflags;
