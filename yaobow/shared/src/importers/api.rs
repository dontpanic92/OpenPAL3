//! High-level "one call" glTF → target-format-bytes conversion, wiring
//! [`super::load_gltf_scene`], the per-format `convert_with_template`
//! functions, and each format's `fileformats` writer together.
//!
//! Accepts an optional `replacement_template`: the raw bytes of an
//! existing `.mv3`/`.pol`/`.cvd` file in the target format, parsed with
//! the corresponding `fileformats` reader and used as a fallback source
//! for opaque/reserved metadata (texture tables, unknown byte blobs,
//! material colors, ...) whenever the source glTF has no
//! `asset.extras.yaobow` round-trip payload of its own. A real
//! `asset.extras.yaobow` payload always takes precedence over the
//! template — see each converter's `convert_with_template` doc comment.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use super::cvd;
use super::error::{Diagnostics, ImportError};
use super::loader::load_gltf_scene;
use super::mv3;
use super::pol;
use super::target::{ImportOptions, TargetFormat};

#[derive(Debug, Clone)]
pub struct ConvertedTexture {
    /// Model-relative target path, e.g. `_yaobow_import/diffuse.tga`.
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct GltfImportBundle {
    pub model_bytes: Vec<u8>,
    pub textures: Vec<ConvertedTexture>,
}

/// Loads `path` (a `.glb`/`.gltf` file), converts it to `target`, and
/// serializes the result, returning the raw output bytes plus every
/// [`Diagnostics`] message collected along the way (from loading *and*
/// converting).
///
/// `replacement_template`, if given, must be the raw bytes of an existing
/// file in `target`'s format (e.g. an `.mv3` file when `target` is
/// [`TargetFormat::Mv3`]); it's parsed with the matching `fileformats`
/// reader and its opaque metadata is used as described in the module
/// docs. Pass `None` to always use plain defaults instead.
pub fn convert_gltf_to_bytes(
    path: impl AsRef<Path>,
    target: TargetFormat,
    options: &ImportOptions,
    replacement_template: Option<&[u8]>,
) -> Result<(Vec<u8>, Diagnostics), ImportError> {
    let (bundle, diagnostics) =
        convert_gltf_to_bundle(path, target, options, replacement_template)?;
    if !bundle.textures.is_empty() {
        return Err(ImportError::Other(
            "textured glTF conversion produces multiple files; use convert_gltf_to_bundle"
                .to_string(),
        ));
    }
    Ok((bundle.model_bytes, diagnostics))
}

/// Converts a glTF source into the target model plus every referenced
/// base-color texture, re-encoded as TGA under `_yaobow_import/`.
pub fn convert_gltf_to_bundle(
    path: impl AsRef<Path>,
    target: TargetFormat,
    options: &ImportOptions,
    replacement_template: Option<&[u8]>,
) -> Result<(GltfImportBundle, Diagnostics), ImportError> {
    let path = path.as_ref();
    let model_stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "model".to_string());
    let texture_directory = format!("_yaobow_import/{model_stem}");
    convert_gltf_to_bundle_in_directory(
        path,
        target,
        options,
        replacement_template,
        &texture_directory,
    )
}

pub fn convert_gltf_to_bundle_in_directory(
    path: impl AsRef<Path>,
    target: TargetFormat,
    options: &ImportOptions,
    replacement_template: Option<&[u8]>,
    texture_directory: &str,
) -> Result<(GltfImportBundle, Diagnostics), ImportError> {
    let mut components = Path::new(texture_directory).components();
    if texture_directory.is_empty()
        || components.any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(ImportError::Other(format!(
            "texture output directory must be a non-empty relative path: {texture_directory}"
        )));
    }

    let (mut scene, mut diagnostics) = load_gltf_scene(path)?;
    remap_texture_directory(&mut scene, texture_directory);

    let (bytes, convert_diagnostics) = match target {
        TargetFormat::Mv3 => {
            let template = replacement_template
                .map(|bytes| fileformats::mv3::read_mv3(&mut Cursor::new(bytes)))
                .transpose()
                .map_err(|err| ImportError::TemplateRead(err.to_string()))?;
            let (file, diag) = mv3::convert_with_template(&scene, options, template.as_ref())?;
            let mut buf = Vec::new();
            fileformats::mv3::write_mv3(&mut Cursor::new(&mut buf), &file)?;
            (buf, diag)
        }
        TargetFormat::Pol => {
            let template = replacement_template
                .map(|bytes| fileformats::pol::read_pol(&mut Cursor::new(bytes)))
                .transpose()
                .map_err(|err| ImportError::TemplateRead(err.to_string()))?;
            let (file, diag) = pol::convert_with_template(&scene, options, template.as_ref())?;
            let mut buf = Vec::new();
            fileformats::pol::write_pol(&mut Cursor::new(&mut buf), &file)?;
            (buf, diag)
        }
        TargetFormat::Cvd => {
            let template = replacement_template
                .map(|bytes| fileformats::pal3::cvd::read_cvd(&mut Cursor::new(bytes)))
                .transpose()
                .map_err(|err| ImportError::TemplateRead(err.to_string()))?;
            let (file, diag) = cvd::convert_with_template(&scene, options, template.as_ref())?;
            let mut buf = Vec::new();
            fileformats::pal3::cvd::write_cvd(&mut buf, &file)?;
            (buf, diag)
        }
    };

    diagnostics.0.extend(convert_diagnostics.0);
    let textures = scene
        .textures
        .iter()
        .map(|texture| ConvertedTexture {
            relative_path: texture.relative_path.clone(),
            bytes: texture.bytes.clone(),
        })
        .collect();
    Ok((
        GltfImportBundle {
            model_bytes: bytes,
            textures,
        },
        diagnostics,
    ))
}

fn remap_texture_directory(scene: &mut super::scene::ImportedScene, texture_directory: &str) {
    let mut remapped = HashMap::new();
    for texture in &mut scene.textures {
        let old = texture.relative_path.clone();
        let file_name = Path::new(&old)
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        texture.relative_path = format!("{texture_directory}/{file_name}");
        remapped.insert(old, texture.relative_path.clone());
    }
    for node in &mut scene.nodes {
        let Some(mesh) = &mut node.mesh else {
            continue;
        };
        for primitive in &mut mesh.primitives {
            let Some(texture) = &primitive.material_texture else {
                continue;
            };
            if let Some(new_path) = remapped.get(texture) {
                primitive.material_texture = Some(new_path.clone());
            }
        }
    }
}
