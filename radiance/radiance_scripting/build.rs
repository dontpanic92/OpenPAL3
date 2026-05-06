use std::path::PathBuf;

fn main() {
    generate_pair("scripting.idl", "scripting_comdef.rs", "scripting.p7");
    generate_pair(
        "editor_services.idl",
        "services_comdef.rs",
        "editor_services.p7",
    );
    generate_p7("radiance.idl", "radiance.p7");
    generate_p7("editor.idl", "editor.p7");
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
    generate_rust(idl_file, rust_out);
    generate_p7(idl_file, p7_out);
}

fn generate_rust(idl_file: &str, out_file: &str) {
    let idl = idl_path(idl_file);
    let out = out_path(out_file);
    let dependencies = crosscom_ccidl::generate_to_file(&idl, &out)
        .unwrap_or_else(|err| panic!("Failed to generate {}: {}", out_file, err));
    for dependency in dependencies {
        println!("cargo:rerun-if-changed={}", dependency.display());
    }
}

fn generate_p7(idl_file: &str, out_file: &str) {
    let idl = idl_path(idl_file);
    let out = out_path(out_file);
    let dependencies = crosscom_ccidl::generate_protosept_to_file(&idl, &out)
        .unwrap_or_else(|err| panic!("Failed to generate {}: {}", out_file, err));
    for dependency in dependencies {
        println!("cargo:rerun-if-changed={}", dependency.display());
    }
}
