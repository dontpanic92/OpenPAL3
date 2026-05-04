use std::path::PathBuf;

fn main() {
    generate_comdef("yaobow_editor.idl", "yaobow_editor_comdef.rs");
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
