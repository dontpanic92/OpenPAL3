use std::path::PathBuf;

fn main() {
    generate_p7("crosscom.idl", "crosscom.p7");
    generate_pair(
        "scripting_services.idl",
        "services_comdef.rs",
        "scripting_services.p7",
    );
    generate_p7("radiance.idl", "radiance.p7");
    generate_p7("editor.idl", "editor.p7");

    // Script bridges (Rust glue produced by ccidl from
    // `[protosept(scriptable)]` annotations). Each bridge file is
    // `include!`'d from `src/comdef/script_bridges.rs` so that the
    // emitted `register_<i>_proto()` / `wrap_<i>()` helpers live in
    // a single crate-local module.
    generate_script_bridge("crosscom.idl", "crosscom_bridge.rs");
    generate_script_bridge("radiance.idl", "radiance_bridge.rs");
    generate_script_bridge("scripting_services.idl", "scripting_services_bridge.rs");

    pack_script_bundle();
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

fn generate_script_bridge(idl_file: &str, out_file: &str) {
    let idl = idl_path(idl_file);
    let out = out_path(out_file);
    let dependencies = crosscom_ccidl::generate_script_bridge_to_file(
        &idl,
        &out,
        "radiance_scripting",
        "script_bridges",
    )
    .unwrap_or_else(|err| panic!("Failed to generate bridge {}: {}", out_file, err));
    for dependency in dependencies {
        println!("cargo:rerun-if-changed={}", dependency.display());
    }
}

/// Pack `scripts/` (currently just `freeview.p7`) into
/// `OUT_DIR/radiance_scripting.ypk`. Loaded at runtime by
/// `radiance_scripting::script_bundle()`.
///
/// Module-bundle package (no root) — this crate doesn't own a
/// `script_app_root`; it just contributes the `freeview` sibling module
/// that per-game launchers `import freeview;`.
fn pack_script_bundle() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let scripts_dir = manifest_dir.join("scripts");
    let out = out_path("radiance_scripting.ypk");

    script_package::pack(
        &script_package::PackInput {
            scripts_dir: &scripts_dir,
            root_entry: None,
            root_name: None,
            extra_files: &[],
        },
        &out,
    )
    .unwrap_or_else(|err| panic!("Failed to pack radiance_scripting scripts: {err}"));
}
