#![allow(unused_variables)]
#![allow(dead_code)]

mod application;
mod comdef;
mod opengujian;
mod openpal3;
mod openpal4;

#[cfg_attr(target_os = "android", ndk_glue::main(backtrace = "on"))]
pub fn android_entry() {
    openpal3::run_openpal3();
}
