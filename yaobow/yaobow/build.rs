use vergen::EmitBuilder;
use std::path::PathBuf;

mod features;

fn main() {
    features::enable_features();
    generate_comdef("yaobow.idl", "yaobow_comdef.rs");
    let _ = EmitBuilder::builder().all_git().emit();

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    match target_os.as_str() {
        "android" => println!("cargo:rustc-link-lib=OpenSLES"),
        _ => (),
    };
}

fn generate_comdef(idl_file: &str, out_file: &str) {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let idl_path = workspace_root
        .join("crosscom")
        .join("idl")
        .join(idl_file);
    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join(out_file);
    let dependencies = crosscom_ccidl::generate_to_file(&idl_path, &out_path)
        .unwrap_or_else(|err| panic!("Failed to generate {}: {}", out_file, err));

    for dependency in dependencies {
        println!("cargo:rerun-if-changed={}", dependency.display());
    }
}
