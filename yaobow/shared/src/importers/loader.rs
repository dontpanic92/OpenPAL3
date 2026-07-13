//! glTF → [`super::scene::ImportedScene`] loader.
//!
//! Accepts `.glb` (binary, magic-sniffed) and `.gltf` (JSON) uniformly via
//! [`gltf::Gltf::from_slice`]. Buffers must be the GLB's embedded `BIN`
//! chunk or **relative file paths** next to the source document — data
//! URIs and any URI with a network scheme (`http://`, `https://`, ...)
//! are rejected with [`ImportError::UnsupportedSource`], since PAL3 assets
//! are always shipped as loose files next to the model. A relative path
//! is only accepted if it stays contained beneath the glTF file's
//! directory (`base_dir`): absolute paths, Windows drive/UNC prefixes,
//! and `..` traversal are rejected outright by
//! [`reject_unsafe_relative_path`], and (since a buffer must exist to be
//! read) the resolved path is additionally canonicalized and checked to
//! still be beneath `base_dir` by [`ensure_within_base_dir`], which also
//! defeats a symlink planted inside `base_dir` pointing back out of it.
//!
//! Images follow the same relative-file-path/GLB-BIN-chunk rule when
//! possible, but every current target format only needs a texture *name*
//! (never decoded pixels), so a `bufferView`-embedded image (common in
//! single-file GLBs) doesn't have to fail the whole import: this loader
//! resolves it to a deterministic placeholder name and records a
//! diagnostic, so a round-tripped [`super::extras::YaobowExtras`] texture
//! name (checked later, per target format) or a manual rename can still
//! recover the real name. Image URIs get the same path-safety checks as
//! buffers, except the canonical-containment check only applies if a
//! file actually exists at the resolved path (an image reference never
//! has to exist on disk here, only its name is read).

use std::path::{Path, PathBuf};

use gltf::animation::util::ReadOutputs;
use gltf::mesh::Mode;

use super::error::{Diagnostics, ImportError};
use super::extras::parse_yaobow_extras;
use super::scene::{
    ImportedAnimation, ImportedMesh, ImportedMorphTarget, ImportedNode, ImportedPrimitive,
    ImportedScene, ImportedTrsChannel, ImportedWeightsChannel, Interpolation, TrsProperty,
};

/// Loads a `.glb`/`.gltf` file from disk into a normalized [`ImportedScene`]
/// plus any non-fatal [`Diagnostics`] collected along the way (e.g. a
/// placeholder name substituted for a bufferView-embedded image).
pub fn load_gltf_scene(
    path: impl AsRef<Path>,
) -> Result<(ImportedScene, Diagnostics), ImportError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|source| ImportError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let gltf = gltf::Gltf::from_slice(&bytes)?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    load_gltf_scene_from(&gltf, base_dir)
}

/// Loads an already-parsed [`gltf::Gltf`] (JSON + optional GLB blob) into a
/// normalized [`ImportedScene`], resolving external buffers relative to
/// `base_dir`. Exposed separately from [`load_gltf_scene`] so tests (and
/// any future in-memory caller) can skip the file-system round trip.
pub fn load_gltf_scene_from(
    gltf: &gltf::Gltf,
    base_dir: &Path,
) -> Result<(ImportedScene, Diagnostics), ImportError> {
    let mut diagnostics = Diagnostics::default();
    let document = &gltf.document;
    let buffer_data = load_buffers(document, gltf.blob.as_deref(), base_dir)?;

    let mut nodes = Vec::with_capacity(document.nodes().count());
    for node in document.nodes() {
        nodes.push(load_node(&node, &buffer_data, base_dir, &mut diagnostics)?);
    }

    let roots: Vec<usize> = match document.default_scene() {
        Some(scene) => scene.nodes().map(|n| n.index()).collect(),
        None => document
            .scenes()
            .next()
            .map(|scene| scene.nodes().map(|n| n.index()).collect())
            .unwrap_or_default(),
    };

    let mut animations = Vec::with_capacity(document.animations().count());
    for animation in document.animations() {
        animations.push(load_animation(&animation, &buffer_data)?);
    }

    let extras = parse_yaobow_extras(
        document
            .as_json()
            .asset
            .extras
            .as_deref()
            .map(|raw| raw.get()),
    );

    Ok((
        ImportedScene {
            nodes,
            roots,
            animations,
            extras,
        },
        diagnostics,
    ))
}

/// A buffer-view accessor reader closure suitable for
/// `Primitive::reader`/`Channel::reader`, backed by pre-loaded buffer
/// bytes. Built fresh (and used immediately) wherever an accessor needs
/// reading, rather than threaded through as a named generic parameter, to
/// keep the lifetime bookkeeping simple.
fn buffer_reader<'a>(
    buffer_data: &'a [Vec<u8>],
) -> impl Fn(gltf::Buffer<'_>) -> Option<&'a [u8]> + Clone {
    move |buffer: gltf::Buffer| buffer_data.get(buffer.index()).map(|v| v.as_slice())
}

fn load_node(
    node: &gltf::Node,
    buffer_data: &[Vec<u8>],
    base_dir: &Path,
    diagnostics: &mut Diagnostics,
) -> Result<ImportedNode, ImportError> {
    let (translation, rotation, scale) = node.transform().decomposed();
    // Node/mesh/animation display names are only available via the `gltf`
    // crate's `names` feature, which flips on a matching `gltf-json`
    // feature that adds a `name` field to `gltf_json::{Scene, Mesh,
    // Animation, Buffer, ...}` — a field the concurrently-developed glTF
    // *exporter* (`crate::exporters::gltf`) doesn't populate in its struct
    // literals, so enabling it here breaks that crate-wide feature
    // unification. This importer intentionally does not depend on
    // `names` and uses synthetic, index-based names instead; see the
    // module docs and this crate's `Cargo.toml` comment for the (parent
    // adaptation) alternative of adding `name: None` everywhere in
    // `exporters::gltf` so `names` can be shared.
    let name = format!("node{}", node.index());

    let mesh = node
        .mesh()
        .map(|mesh| load_mesh(&mesh, buffer_data, base_dir, diagnostics))
        .transpose()?;

    let extras = node
        .extras()
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw.get()).ok());

    Ok(ImportedNode {
        name,
        children: node.children().map(|c| c.index()).collect(),
        translation,
        rotation,
        scale,
        mesh,
        extras,
    })
}

fn load_mesh(
    mesh: &gltf::Mesh,
    buffer_data: &[Vec<u8>],
    base_dir: &Path,
    diagnostics: &mut Diagnostics,
) -> Result<ImportedMesh, ImportError> {
    let name = format!("mesh{}", mesh.index());

    let mut primitives = Vec::with_capacity(mesh.primitives().count());
    for primitive in mesh.primitives() {
        primitives.push(load_primitive(
            &name,
            &primitive,
            buffer_data,
            base_dir,
            diagnostics,
        )?);
    }

    Ok(ImportedMesh { name, primitives })
}

fn load_primitive(
    mesh_name: &str,
    primitive: &gltf::Primitive,
    buffer_data: &[Vec<u8>],
    base_dir: &Path,
    diagnostics: &mut Diagnostics,
) -> Result<ImportedPrimitive, ImportError> {
    if primitive.mode() != Mode::Triangles {
        return Err(ImportError::UnsupportedTopology {
            mesh: mesh_name.to_string(),
            primitive: primitive.index(),
            mode: primitive.mode(),
        });
    }

    let reader = primitive.reader(buffer_reader(buffer_data));

    let positions: Vec<[f32; 3]> = reader
        .read_positions()
        .ok_or_else(|| ImportError::MissingAttribute {
            mesh: mesh_name.to_string(),
            primitive: primitive.index(),
            attribute: "POSITION",
        })?
        .collect();

    let normals: Vec<[f32; 3]> = reader
        .read_normals()
        .map(|it| it.collect())
        .unwrap_or_default();

    let uv0: Vec<[f32; 2]> = reader
        .read_tex_coords(0)
        .ok_or_else(|| ImportError::MissingAttribute {
            mesh: mesh_name.to_string(),
            primitive: primitive.index(),
            attribute: "TEXCOORD_0",
        })?
        .into_f32()
        .collect();

    let indices: Vec<u32> = reader
        .read_indices()
        .ok_or_else(|| ImportError::MissingIndices {
            mesh: mesh_name.to_string(),
            primitive: primitive.index(),
        })?
        .into_u32()
        .collect();

    // Every index must address a loaded position: a corrupted/malicious
    // glTF document can declare an index accessor with values beyond the
    // vertex count without failing glTF-level validation (accessor
    // bounds are checked against the *buffer*, not against sibling
    // accessors), so this has to be checked explicitly here rather than
    // relying on the `gltf` crate's own validation.
    for &index in &indices {
        if index as usize >= positions.len() {
            return Err(ImportError::PrimitiveIndexOutOfBounds {
                mesh: mesh_name.to_string(),
                primitive: primitive.index(),
                index,
                vertex_count: positions.len(),
            });
        }
    }

    let material = primitive.material();
    let material_alpha_blend = material.alpha_mode() == gltf::material::AlphaMode::Blend;
    let material_texture = material
        .pbr_metallic_roughness()
        .base_color_texture()
        .map(|info| info.texture().source())
        .map(|image| {
            image_texture_name(&image, mesh_name, primitive.index(), base_dir, diagnostics)
        })
        .transpose()?;

    let mut morph_targets = Vec::new();
    for (positions_iter, normals_iter, _tangents_iter) in reader.read_morph_targets() {
        let position_deltas: Vec<[f32; 3]> = positions_iter
            .ok_or_else(|| ImportError::MissingAttribute {
                mesh: mesh_name.to_string(),
                primitive: primitive.index(),
                attribute: "morph target POSITION",
            })?
            .collect();
        let normal_deltas = normals_iter.map(|it| it.collect());
        morph_targets.push(ImportedMorphTarget {
            position_deltas,
            normal_deltas,
        });
    }

    Ok(ImportedPrimitive {
        positions,
        normals,
        uv0,
        indices,
        material_texture,
        material_alpha_blend,
        morph_targets,
    })
}

fn load_animation(
    animation: &gltf::Animation,
    buffer_data: &[Vec<u8>],
) -> Result<ImportedAnimation, ImportError> {
    let name = format!("animation{}", animation.index());

    let mut weight_channels = Vec::new();
    let mut trs_channels = Vec::new();

    for channel in animation.channels() {
        let target = channel.target();
        let node = target.node();
        let node_name = format!("node{}", node.index());

        let interpolation = match channel.sampler().interpolation() {
            gltf::animation::Interpolation::Linear => Interpolation::Linear,
            gltf::animation::Interpolation::Step => Interpolation::Step,
            other @ gltf::animation::Interpolation::CubicSpline => {
                return Err(ImportError::UnsupportedInterpolation {
                    animation: name.clone(),
                    node: node_name,
                    interpolation: other,
                    target: "the normalized glTF importer (no tangent/cubic-spline support)",
                });
            }
        };

        let reader = channel.reader(buffer_reader(buffer_data));
        let times: Vec<f32> = reader
            .read_inputs()
            .map(|it| it.collect())
            .unwrap_or_default();
        let Some(outputs) = reader.read_outputs() else {
            continue;
        };

        match outputs {
            ReadOutputs::Translations(it) => {
                let values = it.map(|[x, y, z]| [x, y, z, 0.0]).collect();
                trs_channels.push(ImportedTrsChannel {
                    node: node.index(),
                    property: TrsProperty::Translation,
                    times,
                    values,
                    interpolation,
                });
            }
            ReadOutputs::Scales(it) => {
                let values = it.map(|[x, y, z]| [x, y, z, 0.0]).collect();
                trs_channels.push(ImportedTrsChannel {
                    node: node.index(),
                    property: TrsProperty::Scale,
                    times,
                    values,
                    interpolation,
                });
            }
            ReadOutputs::Rotations(rotations) => {
                let values: Vec<[f32; 4]> = rotations.into_f32().collect();
                trs_channels.push(ImportedTrsChannel {
                    node: node.index(),
                    property: TrsProperty::Rotation,
                    times,
                    values,
                    interpolation,
                });
            }
            ReadOutputs::MorphTargetWeights(weights) => {
                let flat: Vec<f32> = weights.into_f32().collect();
                let target_count = node
                    .mesh()
                    .and_then(|mesh| mesh.weights().map(|w| w.len()))
                    .or_else(|| {
                        node.mesh()
                            .and_then(|mesh| mesh.primitives().next())
                            .map(|p| p.morph_targets().count())
                    })
                    .unwrap_or_else(|| {
                        if times.is_empty() {
                            0
                        } else {
                            flat.len() / times.len()
                        }
                    });
                weight_channels.push(ImportedWeightsChannel {
                    node: node.index(),
                    times,
                    weights: flat,
                    target_count,
                    interpolation,
                });
            }
        }
    }

    Ok(ImportedAnimation {
        name,
        weight_channels,
        trs_channels,
    })
}

/// Resolves the base color texture's file name from an `image::Image`,
/// percent-decoding the URI. Relative file-path images resolve directly;
/// a `bufferView`-embedded image (no on-disk name at all, common in
/// single-file GLBs) resolves to a deterministic placeholder
/// (`embedded_image_{index}.{ext}`) with a diagnostic instead of failing
/// the import — every current target format only needs a texture *name*,
/// and a Yaobow-extras-provided name (checked later, per target format;
/// see e.g. `importers::pol`'s `texture_names` extras field) takes
/// precedence over this placeholder whenever the source glTF is a
/// round-tripped Yaobow export. Data URIs and remote (network-scheme)
/// URIs are still rejected: this loader only ever reads local files.
///
/// The percent-decoded URI is validated the same way as buffer URIs (see
/// [`resolve_and_validate_uri`]): absolute paths, Windows drive/UNC
/// prefixes and `..` traversal are rejected outright, and if a file
/// actually exists at the resolved path, it must canonicalize to
/// somewhere beneath `base_dir` (an image never *has* to exist on disk —
/// only its name is ever read — so a non-existent, but otherwise safe,
/// relative path is still accepted).
fn image_texture_name(
    image: &gltf::Image,
    mesh_name: &str,
    primitive_index: usize,
    base_dir: &Path,
    diagnostics: &mut Diagnostics,
) -> Result<String, ImportError> {
    match image.source() {
        gltf::image::Source::Uri { uri, mime_type } => {
            if is_data_uri(uri) {
                return Err(ImportError::UnsupportedSource(format!(
                    "data URI image (only relative file paths are supported): {}",
                    truncate_for_error(uri)
                )));
            }
            if is_remote_uri(uri) {
                return Err(ImportError::UnsupportedSource(format!(
                    "remote/network URI image (only relative file paths are supported): {}",
                    truncate_for_error(uri)
                )));
            }
            let _ = mime_type;
            let rel = percent_decode(uri);
            reject_unsafe_relative_path(&rel)?;
            let joined = base_dir.join(&rel);
            if joined.exists() {
                ensure_within_base_dir(base_dir, &joined, uri)?;
            }
            Ok(rel)
        }
        gltf::image::Source::View { mime_type, .. } => {
            let ext = extension_for_mime_type(mime_type);
            let name = format!("embedded_image_{}.{ext}", image.index());
            diagnostics.push(format!(
                "mesh `{mesh_name}` primitive #{primitive_index} uses a bufferView-embedded \
                 image (no on-disk name); substituting placeholder texture name `{name}` — \
                 a round-tripped asset.extras.yaobow texture name, if present, will still take \
                 precedence over this placeholder"
            ));
            Ok(name)
        }
    }
}

/// Maps a glTF image `mimeType` to a plausible file extension for the
/// placeholder name in [`image_texture_name`]. Unknown/absent mime types
/// fall back to `png`, matching the format glTF itself defaults to.
fn extension_for_mime_type(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/bmp" => "bmp",
        "image/tga" | "image/x-tga" | "image/x-targa" => "tga",
        _ => "png",
    }
}

/// Loads every glTF buffer referenced by `document` into memory: the GLB
/// `BIN` chunk for [`gltf::buffer::Source::Bin`], or a file read relative
/// to `base_dir` for [`gltf::buffer::Source::Uri`]. Data URIs are
/// rejected (see module docs); this only supports the two source kinds
/// PAL3 assets actually use. External buffer URIs must both look safe
/// (no absolute path/drive prefix/`..` traversal, checked by
/// [`reject_unsafe_relative_path`]) and — since a buffer must exist on
/// disk to be read at all — canonicalize to somewhere beneath `base_dir`
/// (checked by [`ensure_within_base_dir`]), which also defeats a
/// symlink planted inside `base_dir` pointing back out of it.
fn load_buffers(
    document: &gltf::Document,
    blob: Option<&[u8]>,
    base_dir: &Path,
) -> Result<Vec<Vec<u8>>, ImportError> {
    let mut out = Vec::with_capacity(document.buffers().count());
    for buffer in document.buffers() {
        let data = match buffer.source() {
            gltf::buffer::Source::Bin => blob
                .map(|b| b.to_vec())
                .ok_or(ImportError::MissingGlbBlob(buffer.index()))?,
            gltf::buffer::Source::Uri(uri) => {
                if is_data_uri(uri) {
                    return Err(ImportError::UnsupportedSource(format!(
                        "data URI buffer (only relative file paths are supported): {}",
                        truncate_for_error(uri)
                    )));
                }
                if is_remote_uri(uri) {
                    return Err(ImportError::UnsupportedSource(format!(
                        "remote/network URI buffer (only relative file paths are supported): {}",
                        truncate_for_error(uri)
                    )));
                }
                let rel = percent_decode(uri);
                reject_unsafe_relative_path(&rel)?;
                let joined: PathBuf = base_dir.join(&rel);
                let path = ensure_within_base_dir(base_dir, &joined, uri)?;
                std::fs::read(&path).map_err(|source| ImportError::Io { path, source })?
            }
        };

        let expected = buffer.length();
        if data.len() < expected {
            return Err(ImportError::BufferLengthMismatch {
                index: buffer.index(),
                uri: match buffer.source() {
                    gltf::buffer::Source::Bin => "<glb BIN chunk>".to_string(),
                    gltf::buffer::Source::Uri(uri) => uri.to_string(),
                },
                expected,
                actual: data.len(),
            });
        }
        out.push(data);
    }
    Ok(out)
}

fn is_data_uri(uri: &str) -> bool {
    uri.starts_with("data:")
}

/// Whether `uri` uses a network/remote scheme (`http://`, `https://`,
/// `ftp://`, ...) rather than being a plain relative (or absolute local)
/// file path. glTF relative paths never contain `://`, so this is a safe
/// discriminator without a full URI parser.
fn is_remote_uri(uri: &str) -> bool {
    if let Some(scheme_end) = uri.find("://") {
        let scheme = &uri[..scheme_end];
        // A Windows drive letter (`C:\...`) or similar single-character
        // "scheme" before `:` isn't realistically a URI scheme; only
        // treat multi-character alphabetic prefixes as one.
        return scheme.len() > 1 && scheme.chars().all(|c| c.is_ascii_alphanumeric());
    }
    false
}

/// Rejects a percent-decoded relative buffer/image URI that could escape
/// `base_dir`: an absolute path, a Windows drive letter/UNC prefix (`C:`,
/// `\\server\share`, ...), or any `..` parent-directory component. This
/// loader must reject a Windows-authored absolute/UNC path the same way
/// on every host OS, so raw string prefixes are checked directly, since
/// [`Path`]'s component parser only recognizes `\` as a separator (and
/// `C:` as a drive prefix) when actually compiled for Windows.
fn reject_unsafe_relative_path(decoded: &str) -> Result<(), ImportError> {
    let bytes = decoded.as_bytes();
    let has_drive_prefix = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if decoded.starts_with('/') || decoded.starts_with('\\') || has_drive_prefix {
        return Err(ImportError::UnsafeExternalUri {
            uri: decoded.to_string(),
            reason: "absolute path or Windows drive/UNC prefix",
        });
    }

    for component in Path::new(decoded).components() {
        match component {
            std::path::Component::ParentDir => {
                return Err(ImportError::UnsafeExternalUri {
                    uri: decoded.to_string(),
                    reason: "parent-directory (`..`) traversal",
                });
            }
            std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                return Err(ImportError::UnsafeExternalUri {
                    uri: decoded.to_string(),
                    reason: "absolute path",
                });
            }
            _ => {}
        }
    }
    Ok(())
}

/// Canonicalizes `target` (which must already exist) and `base_dir`, and
/// confirms the former is contained within the latter — closing the gap
/// a purely lexical check (see [`reject_unsafe_relative_path`]) leaves
/// open: a symlink inside `base_dir` that points back out of it. `uri`
/// is only used for the error message (the original, still-percent-
/// encoded URI, for readability).
fn ensure_within_base_dir(
    base_dir: &Path,
    target: &Path,
    uri: &str,
) -> Result<PathBuf, ImportError> {
    let base_canonical = base_dir.canonicalize().map_err(|source| ImportError::Io {
        path: base_dir.to_path_buf(),
        source,
    })?;
    let target_canonical = target.canonicalize().map_err(|source| ImportError::Io {
        path: target.to_path_buf(),
        source,
    })?;
    if !target_canonical.starts_with(&base_canonical) {
        return Err(ImportError::UnsafeExternalUri {
            uri: uri.to_string(),
            reason: "resolves outside the glTF file's directory",
        });
    }
    Ok(target_canonical)
}

fn truncate_for_error(s: &str) -> String {
    if s.len() > 60 {
        format!("{}...", &s[..60])
    } else {
        s.to_string()
    }
}

/// Minimal percent-decoder for relative file URIs (glTF paths may contain
/// `%20` for spaces etc.). Not a full RFC 3986 decoder — good enough for
/// file-system-safe relative paths, which is all glTF buffer/image URIs
/// are expected to contain per this loader's supported subset.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importers::test_support::SceneBuilder;

    // -- `reject_unsafe_relative_path` (lexical checks) ----------------

    #[test]
    fn plain_relative_paths_are_allowed() {
        assert!(reject_unsafe_relative_path("model.bin").is_ok());
        assert!(reject_unsafe_relative_path("textures/diffuse.png").is_ok());
        assert!(reject_unsafe_relative_path("a/b/c.bin").is_ok());
    }

    #[test]
    fn parent_dir_traversal_is_rejected() {
        for uri in ["../secret.bin", "sub/../../secret.bin", "a/../../b.bin"] {
            let err = reject_unsafe_relative_path(uri).unwrap_err();
            assert!(
                matches!(err, ImportError::UnsafeExternalUri { .. }),
                "expected UnsafeExternalUri for `{uri}`, got {err:?}"
            );
        }
    }

    #[test]
    fn absolute_unix_path_is_rejected() {
        let err = reject_unsafe_relative_path("/etc/passwd").unwrap_err();
        assert!(matches!(err, ImportError::UnsafeExternalUri { .. }));
    }

    #[test]
    fn windows_drive_and_unc_prefixes_are_rejected() {
        for uri in [
            "C:\\Windows\\System32\\evil.bin",
            "C:evil.bin",
            "\\\\server\\share\\evil.bin",
            "\\evil.bin",
        ] {
            let err = reject_unsafe_relative_path(uri).unwrap_err();
            assert!(
                matches!(err, ImportError::UnsafeExternalUri { .. }),
                "expected UnsafeExternalUri for `{uri}`, got {err:?}"
            );
        }
    }

    /// A scratch directory under the workspace's (gitignored) `target/`
    /// directory — never `/tmp` — for tests that need real files/symlinks
    /// on disk to exercise [`load_gltf_scene`]'s file-based entry point.
    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/importer_loader_test_scratch")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    /// Writes a minimal (no nodes/meshes) `.gltf` JSON document with a
    /// single external buffer at `uri`, next to `base_dir`, and returns
    /// the `.gltf` file's path. `byte_length` is the buffer's declared
    /// `byteLength` (only checked once the URI itself resolves safely
    /// beneath `base_dir`).
    fn write_gltf_with_buffer_uri(
        base_dir: &Path,
        uri: &str,
        byte_length: usize,
    ) -> std::path::PathBuf {
        let doc = serde_json::json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": byte_length, "uri": uri }],
        });
        let path = base_dir.join("model.gltf");
        std::fs::write(&path, serde_json::to_vec(&doc).unwrap()).expect("write .gltf");
        path
    }

    #[test]
    fn buffer_uri_parent_traversal_is_rejected_end_to_end() {
        let base_dir = scratch_dir("buffer_parent_traversal");
        let path = write_gltf_with_buffer_uri(&base_dir, "../outside.bin", 4);

        let err = load_gltf_scene(&path).unwrap_err();
        assert!(
            matches!(err, ImportError::UnsafeExternalUri { .. }),
            "expected UnsafeExternalUri, got {err:?}"
        );
    }

    #[test]
    fn buffer_uri_absolute_path_is_rejected_end_to_end() {
        let base_dir = scratch_dir("buffer_absolute_path");
        let path = write_gltf_with_buffer_uri(&base_dir, "/etc/passwd", 4);

        let err = load_gltf_scene(&path).unwrap_err();
        assert!(
            matches!(err, ImportError::UnsafeExternalUri { .. }),
            "expected UnsafeExternalUri, got {err:?}"
        );
    }

    #[test]
    fn buffer_uri_percent_encoded_traversal_is_rejected_end_to_end() {
        let base_dir = scratch_dir("buffer_percent_encoded_traversal");
        // `%2e%2e` decodes to `..`; the traversal check must run on the
        // decoded string, not the raw (still-encoded) URI.
        let path = write_gltf_with_buffer_uri(&base_dir, "%2e%2e/outside.bin", 4);

        let err = load_gltf_scene(&path).unwrap_err();
        assert!(
            matches!(err, ImportError::UnsafeExternalUri { .. }),
            "expected UnsafeExternalUri, got {err:?}"
        );
    }

    #[test]
    fn buffer_uri_legit_relative_path_loads_successfully() {
        let base_dir = scratch_dir("buffer_legit_relative_path");
        std::fs::create_dir_all(base_dir.join("sub")).expect("create sub dir");
        let data = [1u8, 2, 3, 4];
        std::fs::write(base_dir.join("sub/data.bin"), data).expect("write data.bin");
        let path = write_gltf_with_buffer_uri(&base_dir, "sub/data.bin", data.len());

        let (scene, diagnostics) = load_gltf_scene(&path).expect("load should succeed");
        assert!(scene.nodes.is_empty());
        assert!(diagnostics.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn buffer_uri_symlink_escaping_base_dir_is_rejected() {
        let root = scratch_dir("buffer_symlink_escape");
        let base_dir = root.join("base");
        let outside_dir = root.join("outside");
        std::fs::create_dir_all(&base_dir).expect("create base dir");
        std::fs::create_dir_all(&outside_dir).expect("create outside dir");
        std::fs::write(outside_dir.join("secret.bin"), [9u8; 4]).expect("write secret.bin");
        // A symlink with no literal `..` component in its *name*, but
        // whose target resolves outside `base_dir`: the lexical check in
        // `reject_unsafe_relative_path` can't catch this on its own,
        // which is exactly why `ensure_within_base_dir` canonicalizes
        // and re-checks containment.
        std::os::unix::fs::symlink(outside_dir.join("secret.bin"), base_dir.join("linked.bin"))
            .expect("create symlink");
        let path = write_gltf_with_buffer_uri(&base_dir, "linked.bin", 4);

        let err = load_gltf_scene(&path).unwrap_err();
        assert!(
            matches!(err, ImportError::UnsafeExternalUri { .. }),
            "expected UnsafeExternalUri, got {err:?}"
        );
    }

    #[test]
    fn data_uri_buffer_is_unaffected_by_path_validation() {
        // Data URIs are rejected for an unrelated reason (unsupported
        // source, not a path-safety violation) *before* any percent
        // decoding or path validation ever runs.
        let base_dir = scratch_dir("buffer_data_uri");
        let path = write_gltf_with_buffer_uri(
            &base_dir,
            "data:application/octet-stream;base64,AQIDBA==",
            4,
        );

        let err = load_gltf_scene(&path).unwrap_err();
        assert!(
            matches!(err, ImportError::UnsupportedSource(_)),
            "expected UnsupportedSource, got {err:?}"
        );
    }

    // -- Image URI validation -------------------------------------------

    #[test]
    fn image_uri_parent_traversal_is_rejected() {
        let (positions, normals, uv0, indices) = (
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            None,
            vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            vec![0u16, 1, 2],
        );
        let mut builder = SceneBuilder::new();
        let image = builder.add_image_uri("../outside/evil.png");
        let texture = builder.add_texture(image);
        let material = builder.add_material(Some(texture), false);
        let mesh =
            builder.add_triangle_mesh(&positions, normals, &uv0, &indices, Some(material), &[]);
        let node = builder.add_node(Some(mesh), &[], None, None, None);
        let gltf = builder.parse(&[node]);

        let err = load_gltf_scene_from(&gltf, Path::new(".")).unwrap_err();
        assert!(
            matches!(err, ImportError::UnsafeExternalUri { .. }),
            "expected UnsafeExternalUri, got {err:?}"
        );
    }

    #[test]
    fn image_uri_absolute_path_is_rejected() {
        let mut builder = SceneBuilder::new();
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let uv0 = vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        let indices = vec![0u16, 1, 2];
        let image = builder.add_image_uri("/etc/evil.png");
        let texture = builder.add_texture(image);
        let material = builder.add_material(Some(texture), false);
        let mesh = builder.add_triangle_mesh(&positions, None, &uv0, &indices, Some(material), &[]);
        let node = builder.add_node(Some(mesh), &[], None, None, None);
        let gltf = builder.parse(&[node]);

        let err = load_gltf_scene_from(&gltf, Path::new(".")).unwrap_err();
        assert!(
            matches!(err, ImportError::UnsafeExternalUri { .. }),
            "expected UnsafeExternalUri, got {err:?}"
        );
    }

    #[test]
    fn image_uri_nonexistent_relative_path_still_succeeds() {
        // Images never have to exist on disk (only the *name* is used),
        // so a safe-but-nonexistent relative path must still succeed —
        // this is the same behavior `generic_gltf_to_pol_parse_write_read`
        // in `synthetic_tests.rs` already relies on.
        let mut builder = SceneBuilder::new();
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let uv0 = vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        let indices = vec![0u16, 1, 2];
        let image = builder.add_image_uri("textures/diffuse.png");
        let texture = builder.add_texture(image);
        let material = builder.add_material(Some(texture), false);
        let mesh = builder.add_triangle_mesh(&positions, None, &uv0, &indices, Some(material), &[]);
        let node = builder.add_node(Some(mesh), &[], None, None, None);
        let gltf = builder.parse(&[node]);

        let (scene, diagnostics) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
        assert!(diagnostics.is_empty());
        assert_eq!(
            scene.nodes[0].mesh.as_ref().unwrap().primitives[0]
                .material_texture
                .as_deref(),
            Some("textures/diffuse.png")
        );
    }

    #[cfg(unix)]
    #[test]
    fn image_uri_symlink_escaping_base_dir_is_rejected() {
        let root = scratch_dir("image_symlink_escape");
        let base_dir = root.join("base");
        let outside_dir = root.join("outside");
        std::fs::create_dir_all(&base_dir).expect("create base dir");
        std::fs::create_dir_all(&outside_dir).expect("create outside dir");
        std::fs::write(outside_dir.join("secret.png"), [9u8; 4]).expect("write secret.png");
        std::os::unix::fs::symlink(outside_dir.join("secret.png"), base_dir.join("linked.png"))
            .expect("create symlink");

        let mut builder = SceneBuilder::new();
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let uv0 = vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        let indices = vec![0u16, 1, 2];
        let image = builder.add_image_uri("linked.png");
        let texture = builder.add_texture(image);
        let material = builder.add_material(Some(texture), false);
        let mesh = builder.add_triangle_mesh(&positions, None, &uv0, &indices, Some(material), &[]);
        let node = builder.add_node(Some(mesh), &[], None, None, None);
        let gltf = builder.parse(&[node]);

        let err = load_gltf_scene_from(&gltf, &base_dir).unwrap_err();
        assert!(
            matches!(err, ImportError::UnsafeExternalUri { .. }),
            "expected UnsafeExternalUri, got {err:?}"
        );
    }

    // -- Primitive index bounds checking ---------------------------------

    #[test]
    fn primitive_index_out_of_bounds_is_rejected() {
        let mut builder = SceneBuilder::new();
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let uv0 = vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        // Only 3 positions are declared (indices 0..=2 valid), but the
        // index buffer references index 5 — malformed/corrupted glTF
        // data that must be rejected explicitly rather than causing an
        // out-of-bounds panic/silent misread later in the pipeline.
        let indices = vec![0u16, 1, 5];
        let mesh = builder.add_triangle_mesh(&positions, None, &uv0, &indices, None, &[]);
        let node = builder.add_node(Some(mesh), &[], None, None, None);
        let gltf = builder.parse(&[node]);

        let err = load_gltf_scene_from(&gltf, Path::new(".")).unwrap_err();
        match err {
            ImportError::PrimitiveIndexOutOfBounds {
                index,
                vertex_count,
                ..
            } => {
                assert_eq!(index, 5);
                assert_eq!(vertex_count, 3);
            }
            other => panic!("expected PrimitiveIndexOutOfBounds, got {other:?}"),
        }
    }

    #[test]
    fn primitive_indices_within_bounds_are_accepted() {
        let mut builder = SceneBuilder::new();
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let uv0 = vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        let indices = vec![0u16, 1, 2];
        let mesh = builder.add_triangle_mesh(&positions, None, &uv0, &indices, None, &[]);
        let node = builder.add_node(Some(mesh), &[], None, None, None);
        let gltf = builder.parse(&[node]);

        load_gltf_scene_from(&gltf, Path::new(".")).expect("in-bounds indices should load fine");
    }
}
