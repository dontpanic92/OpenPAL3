//! End-to-end guard for the script-package writer + reader contract:
//! the build-time `script_package::pack` writes a `.ypk` whose layout
//! the engine's canonical `radiance::asset::ypk::YpkArchive` (mounted
//! via `AssetManager::mount_ypk_bytes`) can decode.
//!
//! `script-package` vendors its own `YpkWriter` so build scripts don't
//! drag in the full `radiance` crate. This test guards against wire-
//! format drift between that writer and `radiance::asset::ypk::YpkWriter`.

use std::fs;
use std::path::PathBuf;

use radiance::asset::AssetManager;
use script_package::{ExtraFile, PackInput, pack};

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
    fs::create_dir_all(scripts_dir.join("openpal4")).unwrap();

    fs::write(scripts_dir.join("app.p7"), "pub fn init() -> int { 1 }").unwrap();
    fs::write(scripts_dir.join("title.p7"), "// title module").unwrap();
    fs::write(
        scripts_dir.join("openpal4").join("actor_controller.p7"),
        "// nested actor",
    )
    .unwrap();

    let extra_path = unique_tmp("extra.p7");
    fs::write(&extra_path, "// generated binding").unwrap();

    let out_dir = unique_tmp("out");
    fs::create_dir_all(&out_dir).unwrap();
    let ypk_path = out_dir.join("bundle.ypk");

    pack(
        &PackInput {
            scripts_dir: Some(&scripts_dir),
            extra_files: &[ExtraFile {
                source_path: &extra_path,
                virtual_entry: "svc_gen.p7",
            }],
        },
        &ypk_path,
    )
    .unwrap();

    // Read every byte of the produced ypk into a Vec, then leak it as
    // `&'static [u8]` so we can feed it through `mount_ypk_bytes`
    // (which expects `&'static [u8]` because at runtime the bytes come
    // from `include_bytes!`).
    let bytes = fs::read(&ypk_path).unwrap();
    let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());

    let assets = AssetManager::new();
    assets.mount_ypk_bytes("/pkg", leaked).unwrap();

    assert_eq!(
        assets.read_to_end("/pkg/app.p7").unwrap(),
        b"pub fn init() -> int { 1 }".to_vec()
    );
    assert_eq!(
        assets.read_to_end("/pkg/title.p7").unwrap(),
        b"// title module".to_vec()
    );
    assert_eq!(
        assets
            .read_to_end("/pkg/openpal4/actor_controller.p7")
            .unwrap(),
        b"// nested actor".to_vec()
    );
    assert_eq!(
        assets.read_to_end("/pkg/svc_gen.p7").unwrap(),
        b"// generated binding".to_vec()
    );

    let _ = fs::remove_file(&extra_path);
    let _ = fs::remove_dir_all(&scripts_dir);
    let _ = fs::remove_dir_all(&out_dir);
}
