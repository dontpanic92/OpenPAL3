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

use std::io::Cursor;
use std::path::Path;

use super::cvd;
use super::error::{Diagnostics, ImportError};
use super::loader::load_gltf_scene;
use super::mv3;
use super::pol;
use super::target::{ImportOptions, TargetFormat};

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
    let (scene, mut diagnostics) = load_gltf_scene(path)?;

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
    Ok((bytes, diagnostics))
}
