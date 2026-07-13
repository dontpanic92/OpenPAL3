//! Codegen entry point for this crate's own tiny COM surface (see
//! `idl/yaobow_asset_patcher.idl`): a single `AssetPatcherUiLayer:
//! IUiLayer` class, registered directly with the engine's
//! `UiManager::register_ui_layer` (no director/scene involved).
//!
//! This mirrors the `generate_comdef` pattern used by every other
//! crate's `build.rs` in this workspace (e.g. `yaobow/yaobow/build.rs`,
//! `yaobow/shared/build.rs`), except the source `.idl` file lives
//! inside *this* crate's own `idl/` directory rather than the shared
//! `crosscom/idl/`. The IDL's `import ../../../crosscom/idl/radiance.idl;`
//! reaches the existing shared IDL purely to resolve `IUiLayer` —
//! nothing under `crosscom/idl/` is modified or duplicated.

use std::path::PathBuf;

mod features;

fn main() {
    features::enable_features();
    generate_comdef("yaobow_asset_patcher.idl", "yaobow_asset_patcher_comdef.rs");
}

fn idl_path(idl_file: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    manifest_dir.join("idl").join(idl_file)
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
