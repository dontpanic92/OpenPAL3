use std::path::PathBuf;

mod features;

fn main() {
    features::enable_features();
    generate_comdef("openpal3.idl", "shared_openpal3_comdef.rs");
    generate_p7("openpal3.idl", "shared_openpal3.p7");
    generate_script_bridge("openpal3.idl", "shared_openpal3_bridge.rs");
    generate_comdef("openpal4.idl", "shared_openpal4_comdef.rs");
    generate_p7("openpal4.idl", "shared_openpal4.p7");
    generate_script_bridge("openpal4.idl", "shared_openpal4_bridge.rs");
    generate_comdef("openpal5.idl", "shared_openpal5_comdef.rs");
    generate_p7("openpal5.idl", "shared_openpal5.p7");
    generate_script_bridge("openpal5.idl", "shared_openpal5_bridge.rs");
    generate_comdef("openswd5.idl", "shared_openswd5_comdef.rs");
    generate_p7("openswd5.idl", "shared_openswd5.p7");
    generate_script_bridge("openswd5.idl", "shared_openswd5_bridge.rs");
    generate_comdef("shared_services.idl", "shared_services_comdef.rs");

    // PAL4 debug overlay bridge: emit both the Rust ComObject scaffolding
    // and the p7 binding source consumed by `ScriptHost::add_binding`.
    generate_comdef("pal4_debug.idl", "shared_pal4_debug_comdef.rs");
    generate_p7("pal4_debug.idl", "shared_pal4_debug.p7");
    generate_script_bridge("pal4_debug.idl", "shared_pal4_debug_bridge.rs");

    pack_script_bundle();
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

fn generate_script_bridge(idl_file: &str, out_file: &str) {
    let idl_path = idl_path(idl_file);
    let out_path = out_path(out_file);
    let dependencies = crosscom_ccidl::generate_script_bridge_to_file(
        &idl_path,
        &out_path,
        "shared",
        "script_bridges",
    )
    .unwrap_or_else(|err| panic!("Failed to generate bridge {}: {}", out_file, err));

    for dependency in dependencies {
        println!("cargo:rerun-if-changed={}", dependency.display());
    }
}

/// Pack `scripts/` (currently `openpal4/actor_controller.p7`) plus the
/// five codegen-derived IDL p7s (`shared_openpal{3,4,5}.p7`,
/// `shared_openswd5.p7`, `shared_pal4_debug.p7` from `OUT_DIR`) into
/// `OUT_DIR/shared_scripts.ypk`. The bundle is module-only (no root);
/// the IDL p7s register as `idl_bindings` so app-owned modules can
/// `import openpal3;` etc.
fn pack_script_bundle() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let scripts_dir = manifest_dir.join("scripts");
    let out = out_path("shared_scripts.ypk");

    let extras = [
        ("shared_openpal3.p7", "openpal3"),
        ("shared_openpal4.p7", "openpal4"),
        ("shared_openpal5.p7", "openpal5"),
        ("shared_openswd5.p7", "openswd5"),
        ("shared_pal4_debug.p7", "pal4_debug"),
    ];

    // Stable storage for the absolute paths the ExtraFile borrows.
    let extra_paths: Vec<PathBuf> = extras.iter().map(|(file, _)| out_path(file)).collect();

    let extra_files: Vec<script_package::ExtraFile<'_>> = extras
        .iter()
        .zip(extra_paths.iter())
        .map(|((file, module), path)| script_package::ExtraFile {
            source_path: path.as_path(),
            virtual_entry: file,
            module_name: module,
            kind: script_package::ModuleKind::IdlBinding,
        })
        .collect();

    script_package::pack(
        &script_package::PackInput {
            scripts_dir: &scripts_dir,
            root_entry: None,
            root_name: None,
            extra_files: &extra_files,
        },
        &out,
    )
    .unwrap_or_else(|err| panic!("Failed to pack shared scripts: {err}"));
}
