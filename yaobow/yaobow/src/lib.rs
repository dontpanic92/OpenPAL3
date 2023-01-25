mod comdef;
mod openpal3;
mod openpal4;

use openpal3::openpal3_android_entry;

#[cfg_attr(target_os = "android", ndk_glue::main(backtrace = "on"))]
pub fn android_entry() {
    openpal3_android_entry();
}
