use std::env;

fn main() {
    let target = env::var("TARGET").unwrap();
    if target.contains("msvc") {
        vcpkg::find_package("glib").unwrap();
    } else if target.contains("gnu") {
        let glib_lib_dir = env::var("GLIB_LIB_DIR").unwrap();
        println!("cargo:rustc-link-search={}", glib_lib_dir);
    }
}
