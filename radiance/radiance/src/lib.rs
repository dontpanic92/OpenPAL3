#![allow(unused_variables)]

#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/radiance_comdef.rs"));
}
pub mod application;
pub mod audio;
pub mod components;
pub mod debug;
pub mod imgui;
pub mod input;
pub mod math;
pub mod radiance;
pub mod rendering;
pub mod scene;
pub mod utils;
pub mod video;

mod constants;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate bitflags;
