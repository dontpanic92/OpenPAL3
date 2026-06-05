//! End-to-end guard for the script-package writer + reader contract:
//! the build-time `script_package::pack` writes a `.ypk` that the
//! engine's canonical `radiance::asset::ypk::YpkArchive` can decode
//! into the manifest entries `OwnedScriptPackage` expects.
//!
//! `script-package` vendors its own `YpkWriter` so build scripts don't
//! drag in the full `radiance` crate. This test guards against wire-
//! format drift between that writer and `radiance::asset::ypk::YpkWriter`.

use std::fs;
use std::path::PathBuf;

use radiance_scripting::script_package::OwnedScriptPackage;
use script_package::{ExtraFile, ModuleKind, PackInput, pack};

fn unique_tmp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "yaobow_script_pkg_e2e_{}_{}",
        std::process::id(),
        name
    ))
}

#[test]
fn script_package_round_trips_via_radiance_ypk_archive() {
    let scripts_dir = unique_tmp("src");
    let _ = fs::remove_dir_all(&scripts_dir);
    fs::create_dir_all(scripts_dir.join("idl")).unwrap();

    fs::write(scripts_dir.join("app.p7"), "pub fn init() -> int { 1 }").unwrap();
    fs::write(scripts_dir.join("title.p7"), "// title module").unwrap();
    fs::write(scripts_dir.join("idl").join("svc.p7"), "// idl binding").unwrap();

    let extra_path = unique_tmp("extra.p7");
    fs::write(&extra_path, "// generated binding").unwrap();

    let out_dir = unique_tmp("out");
    fs::create_dir_all(&out_dir).unwrap();
    let ypk_path = out_dir.join("bundle.ypk");

    pack(
        &PackInput {
            scripts_dir: &scripts_dir,
            root_entry: Some("app.p7"),
            root_name: Some("app"),
            extra_files: &[ExtraFile {
                source_path: &extra_path,
                virtual_entry: "extra/svc_gen.p7",
                module_name: "svc_gen",
                kind: ModuleKind::IdlBinding,
            }],
        },
        &ypk_path,
    )
    .unwrap();

    // Read every byte of the produced ypk into a Vec, then leak it as
    // `&'static [u8]` so we can feed it through `from_ypk_bytes` (which
    // takes `&'static [u8]` because at runtime the bytes come from
    // `include_bytes!`).
    let bytes = fs::read(&ypk_path).unwrap();
    let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());

    let pkg = OwnedScriptPackage::from_ypk_bytes(leaked).expect("decode");
    assert_eq!(pkg.root_name.as_deref(), Some("app"));
    assert_eq!(
        pkg.root_source.as_deref().map(|s| s.to_string()),
        Some("pub fn init() -> int { 1 }".to_string())
    );

    let module_names: Vec<&str> = pkg.modules.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(module_names, vec!["title"]);
    assert_eq!(
        pkg.modules[0].source.as_ref(),
        "// title module"
    );

    let idl_names: Vec<&str> = pkg.idl_bindings.iter().map(|m| m.name.as_str()).collect();
    // BTreeMap-backed manifest -> alphabetic.
    assert_eq!(idl_names, vec!["svc", "svc_gen"]);

    let _ = fs::remove_file(&extra_path);
    let _ = fs::remove_dir_all(&scripts_dir);
    let _ = fs::remove_dir_all(&out_dir);
}
