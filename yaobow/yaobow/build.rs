use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    match target_os.as_str() {
        "android" => println!("cargo:rustc-link-lib=OpenSLES"),
        _ => (),
    };
}
