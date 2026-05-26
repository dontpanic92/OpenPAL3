//! Texture resolution and embedding for the glTF exporters.
//!
//! Mirrors the resolution rules used by the in-game loaders so the
//! exported file references the same texture the user sees in the
//! editor preview:
//!
//! 1. Drop the original extension and try `.dds` in the model
//!    directory (PAL3 ships almost all textures as DDS even when the
//!    `.pol`/`.cvd` file references them by another extension).
//! 2. Fall back to the original filename if no `.dds` exists.
//!
//! The raw bytes are then sniffed by magic header:
//!
//! * PNG / JPEG → embedded verbatim with the matching MIME type
//!   (glTF 2.0 mandates one of these two for embedded images).
//! * DDS / TGA / anything else → decoded by the `image` crate and
//!   re-encoded as PNG. DDS and TGA support is what the runtime
//!   relies on (see `SimpleMaterialDef::create`), so the surface
//!   matches.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use mini_fs::{MiniFs, StoreExt};

use super::glb::GlbBuilder;

/// Resolve `texture_name` against `model_dir` using the PAL3 lookup
/// rules and return `(mime, bytes)` ready to be stored as a glTF
/// `Image`. Returns `None` if the texture cannot be found or decoded;
/// callers typically swap in a 1×1 placeholder so the export still
/// loads in DCC tools.
pub fn resolve_texture_bytes(
    vfs: &MiniFs,
    model_dir: &Path,
    texture_name: &str,
) -> Option<(String, Vec<u8>)> {
    let raw = read_with_fallback(vfs, model_dir, texture_name)?;

    // Pass-through fast paths.
    if raw.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Some(("image/png".to_string(), raw));
    }
    if raw.starts_with(&[0xFF, 0xD8]) {
        return Some(("image/jpeg".to_string(), raw));
    }

    // Decode (DDS / TGA / BMP / …) and re-encode as PNG so the result
    // can sit in a glTF 2.0 image directly.
    let img = image::load_from_memory(&raw)
        .or_else(|_| image::load_from_memory_with_format(&raw, image::ImageFormat::Tga))
        .ok()?;
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageOutputFormat::Png).ok()?;
    Some(("image/png".to_string(), out.into_inner()))
}

/// Convenience: resolve + embed the bytes into the builder's BIN blob
/// and return the glTF `Image` index. Returns `None` when the texture
/// cannot be located/decoded.
pub fn embed_texture(
    builder: &mut GlbBuilder,
    vfs: &MiniFs,
    model_dir: &Path,
    texture_name: &str,
) -> Option<gltf_json::Index<gltf_json::Image>> {
    let (mime, bytes) = resolve_texture_bytes(vfs, model_dir, texture_name)?;
    Some(builder.push_image(&bytes, &mime))
}

fn read_with_fallback(vfs: &MiniFs, model_dir: &Path, texture_name: &str) -> Option<Vec<u8>> {
    // Same swap-extension-to-.dds logic as cvd_entity::load_texture
    // and the pol loader.
    let stem = Path::new(texture_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(texture_name);
    let dds_name = format!("{}.dds", stem);

    let mut try_path = PathBuf::from(model_dir);
    try_path.push(&dds_name);
    if let Some(bytes) = read_all(vfs, &try_path) {
        return Some(bytes);
    }

    try_path.pop();
    try_path.push(texture_name);
    read_all(vfs, &try_path)
}

fn read_all(vfs: &MiniFs, path: &Path) -> Option<Vec<u8>> {
    let mut file = vfs.open(path).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(buf)
}
