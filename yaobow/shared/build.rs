use std::path::PathBuf;

mod features;

fn main() {
    features::enable_features();
    generate_comdef("openpal3.idl", "shared_openpal3_comdef.rs");
    generate_comdef("openpal4.idl", "shared_openpal4_comdef.rs");
    generate_comdef("openpal5.idl", "shared_openpal5_comdef.rs");
    generate_comdef("openswd5.idl", "shared_openswd5_comdef.rs");

    // PAL4 debug overlay bridge: emit both the Rust ComObject scaffolding
    // and the p7 binding source consumed by `ScriptHost::add_binding`.
    generate_comdef("pal4_debug.idl", "shared_pal4_debug_comdef.rs");
    generate_p7("pal4_debug.idl", "shared_pal4_debug.p7");
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
    let idl_path = idl_path(idl_file);
    let out_path = out_path(out_file);
    let dependencies = crosscom_ccidl::generate_to_file(&idl_path, &out_path)
        .unwrap_or_else(|err| panic!("Failed to generate {}: {}", out_file, err));

    for dependency in dependencies {
        println!("cargo:rerun-if-changed={}", dependency.display());
    }
}

fn generate_p7(idl_file: &str, out_file: &str) {
    let idl_path = idl_path(idl_file);
    let out_path = out_path(out_file);
    let dependencies = crosscom_ccidl::generate_protosept_to_file(&idl_path, &out_path)
        .unwrap_or_else(|err| panic!("Failed to generate {}: {}", out_file, err));

    for dependency in dependencies {
        println!("cargo:rerun-if-changed={}", dependency.display());
    }
}
