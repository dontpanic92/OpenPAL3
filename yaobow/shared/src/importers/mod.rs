//! Normalized glTF import and PAL3 target-format (`.mv3`/`.pol`/`.cvd`)
//! conversion.
//!
//! [`load_gltf_scene`]/[`load_gltf_scene_from`] parse a `.glb`/`.gltf`
//! document into the format-agnostic [`ImportedScene`] IR; [`mv3::convert`]
//! / [`pol::convert`] / [`cvd::convert`] (and their `write` counterparts)
//! turn that IR into the corresponding PAL3 asset format, applying
//! [`TargetFormat`]-specific [`ImportOptions`] and enforcing each format's
//! topology/attribute/index/quantization/animation constraints with
//! explicit [`ImportError`]s. Non-fatal issues (defaults substituted,
//! precision dropped, round-trip metadata ignored, ...) are collected into
//! a [`Diagnostics`] list returned alongside the converted file.
//!
//! Round-tripping a glTF produced by [`crate::exporters::gltf`] recovers
//! the source format's reserved/"unknown" fields via the
//! `asset.extras.yaobow` envelope (see [`extras`]); plain, hand-authored
//! glTF simply gets sensible defaults for those fields.

mod error;
mod extras;
mod loader;
mod scene;
#[cfg(test)]
mod synthetic_tests;
mod target;
#[cfg(test)]
mod test_support;

pub mod api;
pub mod cvd;
pub mod mv3;
pub mod pol;

pub use api::{
    ConvertedTexture, GltfImportBundle, convert_gltf_to_bundle,
    convert_gltf_to_bundle_in_directory, convert_gltf_to_bytes,
};
pub use error::{Diagnostic, Diagnostics, ImportError};
pub use extras::YaobowExtras;
pub use loader::{load_gltf_scene, load_gltf_scene_from};
pub use scene::{
    ImportedAnimation, ImportedJointInfluence, ImportedMesh, ImportedMorphTarget, ImportedNode,
    ImportedPrimitive, ImportedScene, ImportedSkin, ImportedTexture, ImportedTrsChannel,
    ImportedWeightsChannel, Interpolation, TrsProperty,
};
pub use target::{CvdOptions, ImportOptions, Mv3Options, PolOptions, TargetFormat};

/// Bakes a node's static translation/rotation/uniform-scale into a
/// world-space position. Positions are assumed already local-space glTF
/// coordinates; callers with no further hierarchy to compose (MV3, POL)
/// use this directly, while hierarchical targets (CVD) compose it
/// per-ancestor.
pub(crate) fn quantize_world(p: [f32; 3], node: &scene::ImportedNode, scale: f32) -> [f32; 3] {
    let scaled = [p[0] * scale, p[1] * scale, p[2] * scale];
    let rotated = rotate_vec(scaled, node.rotation);
    [
        rotated[0] + node.translation[0],
        rotated[1] + node.translation[1],
        rotated[2] + node.translation[2],
    ]
}

/// Standard (unit) quaternion vector rotation: `v + 2*q.xyz x (q.xyz x v + q.w*v)`.
pub(crate) fn rotate_vec(v: [f32; 3], q: [f32; 4]) -> [f32; 3] {
    let qv = [q[0], q[1], q[2]];
    let uv = cross(qv, v);
    let uuv = cross(qv, uv);
    [
        v[0] + 2.0 * (q[3] * uv[0] + uuv[0]),
        v[1] + 2.0 * (q[3] * uv[1] + uuv[1]),
        v[2] + 2.0 * (q[3] * uv[2] + uuv[2]),
    ]
}

/// Rotation only (no inverse-transpose): every target format that uses
/// this only supports uniform node scale, so a plain rotation keeps
/// normals correct.
pub(crate) fn rotate_normal(n: [f32; 3], q: [f32; 4]) -> [f32; 3] {
    rotate_vec(n, q)
}

pub(crate) fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Requires `node.scale` to be uniform (within a small epsilon), returning
/// the common scale factor. Used by target formats that only support a
/// single scalar scale per mesh/node, not an arbitrary 3-vector.
pub(crate) fn uniform_scale(
    node: &scene::ImportedNode,
    target: &'static str,
) -> Result<f32, ImportError> {
    let [sx, sy, sz] = node.scale;
    if (sx - sy).abs() > 1e-3 || (sy - sz).abs() > 1e-3 {
        return Err(ImportError::NonUniformScale {
            node: node.name.clone(),
            scale: node.scale,
            target,
        });
    }
    Ok(sx)
}

/// Errors if `node_index` is the target of any TRS animation channel.
/// Used by target formats with no representation for an animated node
/// transform (MV3, POL); CVD doesn't need this since it supports node
/// TRS animation directly.
pub(crate) fn assert_no_trs_animation(
    scene: &scene::ImportedScene,
    node_index: usize,
    target: &'static str,
) -> Result<(), ImportError> {
    for anim in &scene.animations {
        for ch in &anim.trs_channels {
            if ch.node == node_index {
                return Err(ImportError::UnsupportedAnimationTarget {
                    animation: anim.name.clone(),
                    node: scene.nodes[node_index].name.clone(),
                    property: match ch.property {
                        scene::TrsProperty::Translation => gltf::animation::Property::Translation,
                        scene::TrsProperty::Rotation => gltf::animation::Property::Rotation,
                        scene::TrsProperty::Scale => gltf::animation::Property::Scale,
                    },
                    target,
                });
            }
        }
    }
    Ok(())
}

/// Indices into `scene.nodes` for the nodes that should be treated as
/// top-level "roots" by target formats with no (or only single-level)
/// hierarchy support, such as [`mv3`] and [`pol`].
///
/// Uses `scene.roots` (the document's default/first scene) when
/// non-empty; otherwise falls back to every node that isn't referenced as
/// a child by any other node, so a document with no `scenes` array at all
/// (unusual, but not invalid glTF) still has a sensible set of roots.
pub(crate) fn effective_roots(scene: &scene::ImportedScene) -> Vec<usize> {
    if !scene.roots.is_empty() {
        return scene.roots.clone();
    }
    let mut is_child = vec![false; scene.nodes.len()];
    for node in &scene.nodes {
        for &child in &node.children {
            if let Some(flag) = is_child.get_mut(child) {
                *flag = true;
            }
        }
    }
    (0..scene.nodes.len()).filter(|&i| !is_child[i]).collect()
}

/// Truncates (on a UTF-8 boundary) `s` to fit in `capacity` bytes and
/// returns it wrapped in a [`fileformats::utils::StringWithCapacity`].
///
/// Writes raw UTF-8 bytes (via `StringWithCapacity`'s `From<T: AsRef
/// <str>>` impl), **not** GBK — every production call site now uses
/// [`gbk_capacity_string`] instead so ASCII/Chinese names round-trip
/// through the GBK-decoding read path correctly; this helper only
/// remains as a test-only convenience for building plain-ASCII
/// synthetic/template fixtures, where UTF-8 and GBK encode identically.
#[cfg(test)]
pub(crate) fn fixed_capacity_string(
    s: &str,
    capacity: usize,
) -> fileformats::utils::StringWithCapacity {
    let mut bytes: Vec<u8> = s.bytes().take(capacity).collect();
    let padded = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(err) => {
            // Truncation landed inside a multi-byte UTF-8 sequence; back
            // off to the last valid boundary instead of panicking.
            let valid_up_to = err.utf8_error().valid_up_to();
            bytes = err.into_bytes();
            bytes.truncate(valid_up_to);
            String::from_utf8(bytes).unwrap_or_default()
        }
    };
    let mut bytes = padded.into_bytes();
    bytes.resize(capacity, 0);
    // SAFETY-free: `\0` padding bytes are valid UTF-8, so this can't fail.
    fileformats::utils::StringWithCapacity::from(String::from_utf8(bytes).unwrap())
}

/// GBK-encodes `s`, matching how every on-disk MV3/POL/CVD reader decodes
/// these fields (see [`fileformats::utils::to_gbk_string`],
/// [`fileformats::utils::SizedString::to_string`],
/// [`fileformats::utils::StringWithCapacity::as_str`]). Returns
/// [`ImportError::StringEncoding`] if `s` contains a character GBK can't
/// represent — writing it as raw UTF-8 instead (the previous behavior,
/// via `SizedString`/`StringWithCapacity`'s blanket `From<T: AsRef<str>>`
/// impl) silently corrupted any non-ASCII (e.g. Chinese) texture/action
/// name on the GBK-decoding read path.
pub(crate) fn gbk_encode(s: &str) -> Result<Vec<u8>, ImportError> {
    use encoding::Encoding;
    encoding::all::GBK
        .encode(s, encoding::EncoderTrap::Strict)
        .map_err(|_| ImportError::StringEncoding(s.to_string()))
}

/// Builds a [`fileformats::utils::SizedString`] from a GBK-encoded `s`.
/// `SizedString` has no fixed on-disk capacity (only a length prefix), so
/// this only fails on an unencodable character, never a length overflow.
pub(crate) fn gbk_sized_string(s: &str) -> Result<fileformats::utils::SizedString, ImportError> {
    Ok(fileformats::utils::SizedString::from_bytes(gbk_encode(s)?))
}

/// Builds a [`fileformats::utils::StringWithCapacity`] from a GBK-encoded
/// `s`, zero-padded to `capacity` bytes. Fails via `err(actual, capacity)`
/// if `s` is unencodable (`actual` is then meaningless and ignored by
/// every current caller) or if its GBK encoding is longer than
/// `capacity`.
pub(crate) fn gbk_capacity_string(
    s: &str,
    capacity: usize,
    err: impl FnOnce(usize, usize) -> ImportError,
) -> Result<fileformats::utils::StringWithCapacity, ImportError> {
    let mut bytes = gbk_encode(s)?;
    if bytes.len() > capacity {
        return Err(err(bytes.len(), capacity));
    }
    bytes.resize(capacity, 0);
    Ok(fileformats::utils::StringWithCapacity::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_capacity_string_pads_short_names() {
        let s = fixed_capacity_string("hi", 5);
        assert_eq!(s.data().len(), 5);
        assert_eq!(s.as_str().unwrap(), "hi");
    }

    #[test]
    fn fixed_capacity_string_truncates_long_names() {
        let s = fixed_capacity_string("this name is too long", 8);
        assert_eq!(s.data().len(), 8);
        assert_eq!(s.as_str().unwrap(), "this nam");
    }
}
