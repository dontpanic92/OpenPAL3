use std::path::PathBuf;

fn main() {
    generate_comdef("yaobow_editor.idl", "yaobow_editor_comdef.rs");
    generate_pair(
        "yaobow_editor_services.idl",
        "yaobow_editor_services_comdef.rs",
        "yaobow_editor_services.p7",
    );
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

fn idl_path(idl_file: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crosscom")
        .join("idl")
        .join(idl_file)
}

fn out_path(out_file: &str) -> PathBuf {
    PathBuf::from(std::env::var("OUT_DIR").unwrap()).join(out_file)
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
