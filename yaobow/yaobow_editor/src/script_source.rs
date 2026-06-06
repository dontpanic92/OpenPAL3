//! Editor p7 script bundle.
//!
//! `scripts/` and the generated bindings (`yaobow_editor_services.p7`,
//! `icons.p7`) are packed at build time by `build.rs` into
//! `OUT_DIR/yaobow_editor.ypk`. [`mount_scripts`] publishes the bytes
//! into a dedicated script `AssetManager` at `/yaobow_editor/`, so
//! scripts can `import yaobow_editor.welcome;`,
//! `import yaobow_editor.icons;`, etc. and the editor app root
//! resolves at `/yaobow_editor/main.p7`.

use radiance::asset::AssetManager;

/// In-binary `.ypk` produced by `build.rs`.
const EDITOR_YPK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/yaobow_editor.ypk"));

/// Mounts this crate's `yaobow_editor.ypk` at `/yaobow_editor/` on
/// the script `AssetManager`.
pub fn mount_scripts(assets: &AssetManager) {
    assets
        .mount_ypk_bytes("/yaobow_editor", EDITOR_YPK)
        .expect("yaobow_editor.ypk must mount");
}
