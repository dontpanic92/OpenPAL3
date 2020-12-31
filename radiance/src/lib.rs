#![feature(arbitrary_self_types)]

#[macro_use]
mod macros;

pub mod application;
pub mod audio;
pub mod input;
pub mod math;
pub mod radiance;
pub mod rendering;
pub mod scene;

mod constants;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate bitflags;
