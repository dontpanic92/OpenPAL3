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

/// Pack the five codegen-derived IDL p7s (`shared_openpal{3,4,5}.p7`,
/// `shared_openswd5.p7`, `shared_pal4_debug.p7` from `OUT_DIR`) into
/// `OUT_DIR/shared_scripts.ypk`. The codegen files are stored under
/// flat names (`openpal3.p7`, `openpal4.p7`, etc.) so that, once the
/// ypk is mounted at `/shared/` on the script `AssetManager`, scripts
/// resolve `import shared.openpal3;` -> `/shared/openpal3.p7`.
///
/// The IDL codegen emits fully-qualified imports directly, driven by
/// each IDL's `module(protosept) shared.X;` directive — no build-time
/// rewrite needed.
fn pack_script_bundle() {
    // `shared` has no authored scripts: `actor_controller.p7` moved to
    // the `yaobow` crate, leaving only codegen-derived IDL p7s from
    // `OUT_DIR`. Pass `scripts_dir: None` so no on-disk directory is
    // required — the bundle is built from `extra_files` alone. When
    // new shared-authored scripts land, create `shared/scripts/` and
    // flip this back to `Some(&scripts_dir)`.
    let out = out_path("shared_scripts.ypk");

    // (OUT_DIR file name, virtual entry inside the ypk).
    let extras = [
        ("shared_openpal3.p7", "openpal3.p7"),
        ("shared_openpal4.p7", "openpal4.p7"),
        ("shared_openpal5.p7", "openpal5.p7"),
        ("shared_openswd5.p7", "openswd5.p7"),
        ("shared_pal4_debug.p7", "pal4_debug.p7"),
    ];

    let extra_paths: Vec<PathBuf> = extras.iter().map(|(file, _)| out_path(file)).collect();

    let extra_files: Vec<script_package::ExtraFile<'_>> = extras
        .iter()
        .zip(extra_paths.iter())
        .map(|((_, virtual_entry), path)| script_package::ExtraFile {
            source_path: path.as_path(),
            virtual_entry,
        })
        .collect();

    script_package::pack(
        &script_package::PackInput {
            scripts_dir: None,
            extra_files: &extra_files,
        },
        &out,
    )
    .unwrap_or_else(|err| panic!("Failed to pack shared scripts: {err}"));
}
