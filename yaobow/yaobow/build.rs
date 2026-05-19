use std::path::PathBuf;
use vergen::EmitBuilder;

mod features;

fn main() {
    features::enable_features();
    generate_comdef("yaobow.idl", "yaobow_comdef.rs");
    generate_pair(
        "yaobow_services.idl",
        "yaobow_services_comdef.rs",
        "yaobow_services.p7",
    );
    let _ = EmitBuilder::builder().all_git().emit();

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    match target_os.as_str() {
        "android" => println!("cargo:rustc-link-lib=OpenSLES"),
        _ => (),
    };
}

fn idl_path(idl_file: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    workspace_root.join("crosscom").join("idl").join(idl_file)
}

fn out_path(out_file: &str) -> PathBuf {
    PathBuf::from(std::env::var("OUT_DIR").unwrap()).join(out_file)
}

fn generate_comdef(idl_file: &str, out_file: &str) {
    let idl = idl_path(idl_file);
    let out = out_path(out_file);
    let dependencies = crosscom_ccidl::generate_to_file(&idl, &out)
        .unwrap_or_else(|err| panic!("Failed to generate {}: {}", out_file, err));

    for dependency in dependencies {
        println!("cargo:rerun-if-changed={}", dependency.display());
    }
}

fn generate_pair(idl_file: &str, rust_out: &str, p7_out: &str) {
    generate_comdef(idl_file, rust_out);
    let idl = idl_path(idl_file);
    let out = out_path(p7_out);
    let dependencies = crosscom_ccidl::generate_protosept_to_file(&idl, &out)
        .unwrap_or_else(|err| panic!("Failed to generate {}: {}", p7_out, err));
    for dependency in dependencies {
        println!("cargo:rerun-if-changed={}", dependency.display());
    }
}
