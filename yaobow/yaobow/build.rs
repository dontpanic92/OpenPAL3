use vergen::EmitBuilder;

mod features;

fn main() {
    features::enable_features();
    let _ = EmitBuilder::builder().all_git().emit();

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    match target_os.as_str() {
        "android" => println!("cargo:rustc-link-lib=OpenSLES"),
        _ => (),
    };
}
