//! glTF → [`super::scene::ImportedScene`] loader.
//!
//! Accepts `.glb` (binary, magic-sniffed) and `.gltf` (JSON) uniformly.
//! Unsupported required `KHR_materials_*` extensions are treated as lossy
//! material hints: the importer keeps the standard base texture/alpha mode
//! and reports a diagnostic instead of rejecting the whole model. All other
//! glTF validation remains enabled, so geometry/compression extensions that
//! the importer cannot decode still fail explicitly. Buffers must be the GLB's
//! embedded `BIN` chunk or **relative file paths** next to the source document — data
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
//! Referenced images are loaded from safe relative paths, GLB buffer views,
//! or image data URIs, decoded, and re-encoded as TGA artifacts under the
//! model-relative `_yaobow_import/` directory. Remote URLs remain rejected.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use gltf::animation::util::ReadOutputs;
use gltf::mesh::Mode;

use super::error::{Diagnostics, ImportError};
use super::extras::parse_yaobow_extras;
use super::scene::{
    ImportedAnimation, ImportedJointInfluence, ImportedMesh, ImportedMorphTarget, ImportedNode,
    ImportedPrimitive, ImportedScene, ImportedSkin, ImportedTexture, ImportedTrsChannel,
    ImportedWeightsChannel, Interpolation, TrsProperty,
};

/// Loads a `.glb`/`.gltf` file from disk into a normalized [`ImportedScene`]
/// plus any non-fatal [`Diagnostics`] collected along the way, including
/// generated TGA texture paths.
pub fn load_gltf_scene(
    path: impl AsRef<Path>,
) -> Result<(ImportedScene, Diagnostics), ImportError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|source| ImportError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let (gltf, ignored_material_extensions) = parse_gltf(&bytes)?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let (scene, mut diagnostics) = load_gltf_scene_from(&gltf, base_dir)?;
    for extension in ignored_material_extensions {
        diagnostics.push(format!(
            "ignored unsupported glTF material extension `{extension}`; imported the material using its standard base texture and alpha mode"
        ));
    }
    Ok((scene, diagnostics))
}

fn parse_gltf(bytes: &[u8]) -> Result<(gltf::Gltf, Vec<String>), ImportError> {
    let gltf::Gltf { document, blob } = gltf::Gltf::from_slice_without_validation(bytes)?;
    let mut json = document.into_json();
    let mut ignored_material_extensions = Vec::new();
    json.extensions_required.retain(|extension| {
        let unsupported = !gltf_json::extensions::ENABLED_EXTENSIONS.contains(&extension.as_str());
        if unsupported && extension.starts_with("KHR_materials_") {
            ignored_material_extensions.push(extension.clone());
            false
        } else {
            true
        }
    });
    let document = gltf::Document::from_json(json)?;
    Ok((gltf::Gltf { document, blob }, ignored_material_extensions))
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
    let mut texture_importer = TextureImporter::new(&buffer_data, base_dir);
    let skins = document
        .skins()
        .map(|skin| load_skin(&skin, &buffer_data))
        .collect::<Result<Vec<_>, _>>()?;

    let mut nodes = Vec::with_capacity(document.nodes().count());
    for node in document.nodes() {
        nodes.push(load_node(
            &node,
            &buffer_data,
            &mut texture_importer,
            &mut diagnostics,
        )?);
    }
    if document
        .as_json()
        .asset
        .generator
        .as_deref()
        .is_some_and(|generator| generator.starts_with("Sketchfab-"))
    {
        // Some Sketchfab FBX conversions store the intended bind cuboid on a
        // transform-only companion node while emitting malformed vertex bounds.
        repair_sketchfab_bind_shapes(&mut nodes, &skins, &mut diagnostics);
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
        animations.push(load_animation(&animation, &buffer_data, &mut diagnostics)?);
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
            skins,
            extras,
            textures: texture_importer.textures,
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
    texture_importer: &mut TextureImporter<'_>,
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
        .map(|mesh| load_mesh(&mesh, buffer_data, texture_importer, diagnostics))
        .transpose()?;
    let morph_weights = node
        .weights()
        .or_else(|| node.mesh().and_then(|mesh| mesh.weights()))
        .map(|weights| weights.to_vec())
        .unwrap_or_default();

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
        skin: node.skin().map(|skin| skin.index()),
        morph_weights,
        extras,
    })
}

fn load_skin(skin: &gltf::Skin, buffer_data: &[Vec<u8>]) -> Result<ImportedSkin, ImportError> {
    let joints: Vec<usize> = skin.joints().map(|joint| joint.index()).collect();
    let inverse_bind_matrices: Vec<[[f32; 4]; 4]> = skin
        .reader(buffer_reader(buffer_data))
        .read_inverse_bind_matrices()
        .map(|matrices| matrices.collect())
        .unwrap_or_else(|| vec![identity_matrix(); joints.len()]);
    if inverse_bind_matrices.len() != joints.len() {
        return Err(ImportError::InverseBindMatrixCountMismatch {
            skin: skin.index(),
            matrices: inverse_bind_matrices.len(),
            joints: joints.len(),
        });
    }
    Ok(ImportedSkin {
        joints,
        inverse_bind_matrices,
    })
}

fn identity_matrix() -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn repair_sketchfab_bind_shapes(
    nodes: &mut [ImportedNode],
    skins: &[ImportedSkin],
    diagnostics: &mut Diagnostics,
) {
    let mut parents = vec![None; nodes.len()];
    for (parent_index, node) in nodes.iter().enumerate() {
        for &child_index in &node.children {
            if let Some(parent) = parents.get_mut(child_index) {
                *parent = Some(parent_index);
            }
        }
    }

    let mut repairs = Vec::new();
    for node_index in 1..nodes.len() {
        let node = &nodes[node_index];
        let Some(skin_index) = node.skin else {
            continue;
        };
        let Some(mesh) = node.mesh.as_ref() else {
            continue;
        };
        let companion_index = node_index - 1;
        let companion = &nodes[companion_index];
        if parents[node_index] != parents[companion_index]
            || companion.mesh.is_some()
            || companion.skin.is_some()
            || !companion.children.is_empty()
            || !is_identity_trs(node)
            || !is_identity_rotation(companion.rotation)
        {
            continue;
        }

        let Some(skin) = skins.get(skin_index) else {
            continue;
        };
        let Some(inverse_bind_scale) = common_inverse_bind_scale(mesh, skin) else {
            continue;
        };
        if companion
            .scale
            .iter()
            .chain(inverse_bind_scale.iter())
            .any(|value| !value.is_finite() || value.abs() <= 1e-6)
        {
            continue;
        }

        let Some((source_min, source_max)) = mesh_position_bounds(mesh) else {
            continue;
        };
        let source_extent = [
            source_max[0] - source_min[0],
            source_max[1] - source_min[1],
            source_max[2] - source_min[2],
        ];
        if source_extent
            .iter()
            .any(|extent| !extent.is_finite() || *extent <= 1e-6)
        {
            continue;
        }
        let target_center = [
            companion.translation[0] / inverse_bind_scale[0],
            companion.translation[1] / inverse_bind_scale[1],
            companion.translation[2] / inverse_bind_scale[2],
        ];
        let target_extent = [
            (companion.scale[0] / inverse_bind_scale[0]).abs(),
            (companion.scale[1] / inverse_bind_scale[1]).abs(),
            (companion.scale[2] / inverse_bind_scale[2]).abs(),
        ];
        if target_center
            .iter()
            .chain(target_extent.iter())
            .any(|value| !value.is_finite())
        {
            continue;
        }

        let source_center = [
            (source_min[0] + source_max[0]) * 0.5,
            (source_min[1] + source_max[1]) * 0.5,
            (source_min[2] + source_max[2]) * 0.5,
        ];
        let changed = (0..3).any(|axis| {
            (source_center[axis] - target_center[axis]).abs() > 1e-3
                || (source_extent[axis] - target_extent[axis]).abs() > 1e-3
        });
        if changed {
            repairs.push((
                node_index,
                companion_index,
                source_center,
                source_extent,
                target_center,
                target_extent,
            ));
        }
    }

    for (node_index, companion_index, source_center, source_extent, target_center, target_extent) in
        repairs
    {
        let mesh = nodes[node_index]
            .mesh
            .as_mut()
            .expect("repair candidate checked for a mesh");
        let mut position_scale = [
            target_extent[0] / source_extent[0],
            target_extent[1] / source_extent[1],
            target_extent[2] / source_extent[2],
        ];
        let inverted_source_winding = mesh.primitives.iter().any(has_inverted_winding);
        let reflection_axis = bind_shape_reflection_axis(
            source_center,
            target_center,
            target_extent,
            inverted_source_winding,
        );
        if let Some(axis) = reflection_axis {
            position_scale[axis] = -position_scale[axis];
        }
        let mut reversed_winding = false;
        for primitive in &mut mesh.primitives {
            for position in &mut primitive.positions {
                for axis in 0..3 {
                    position[axis] = target_center[axis]
                        + (position[axis] - source_center[axis]) * position_scale[axis];
                }
            }
            for target in &mut primitive.morph_targets {
                for delta in &mut target.position_deltas {
                    for axis in 0..3 {
                        delta[axis] *= position_scale[axis];
                    }
                }
                if let Some(normal_deltas) = &mut target.normal_deltas {
                    for (base, delta) in primitive.normals.iter().zip(normal_deltas) {
                        let repaired_base = repair_normal(*base, position_scale);
                        let repaired_target = repair_normal(
                            [base[0] + delta[0], base[1] + delta[1], base[2] + delta[2]],
                            position_scale,
                        );
                        *delta = [
                            repaired_target[0] - repaired_base[0],
                            repaired_target[1] - repaired_base[1],
                            repaired_target[2] - repaired_base[2],
                        ];
                    }
                }
            }
            for normal in &mut primitive.normals {
                *normal = repair_normal(*normal, position_scale);
            }
            if has_inverted_winding(primitive) {
                for triangle in primitive.indices.chunks_exact_mut(3) {
                    triangle.swap(1, 2);
                }
                reversed_winding = true;
            }
        }
        diagnostics.push(format!(
            "repaired nonstandard Sketchfab bind shape for node #{node_index} using companion node #{companion_index}"
        ));
        if let Some(axis) = reflection_axis {
            diagnostics.push(format!(
                "reflected malformed Sketchfab bind-shape {} axis for node #{node_index}",
                ["X", "Y", "Z"][axis]
            ));
        }
        if reversed_winding {
            diagnostics.push(format!(
                "reversed inward-facing Sketchfab triangle winding for node #{node_index}"
            ));
        }
    }
}

fn is_identity_trs(node: &ImportedNode) -> bool {
    node.translation.iter().all(|value| value.abs() <= 1e-6)
        && is_identity_rotation(node.rotation)
        && node.scale.iter().all(|value| (*value - 1.0).abs() <= 1e-6)
}

fn is_identity_rotation(rotation: [f32; 4]) -> bool {
    rotation[0].abs() <= 1e-6
        && rotation[1].abs() <= 1e-6
        && rotation[2].abs() <= 1e-6
        && (rotation[3].abs() - 1.0).abs() <= 1e-6
}

fn common_inverse_bind_scale(mesh: &ImportedMesh, skin: &ImportedSkin) -> Option<[f32; 3]> {
    let mut common: Option<[f32; 3]> = None;
    for influence in mesh
        .primitives
        .iter()
        .filter_map(|primitive| primitive.skin_influences.as_ref())
        .flatten()
        .flatten()
        .filter(|influence| influence.weight > 1e-6)
    {
        let matrix = *skin.inverse_bind_matrices.get(influence.joint as usize)?;
        let scale = axis_aligned_matrix_scale(matrix)?;
        if let Some(common_scale) = common {
            if (0..3).any(|axis| (common_scale[axis] - scale[axis]).abs() > 1e-3) {
                return None;
            }
        } else {
            common = Some(scale);
        }
    }
    common
}

fn axis_aligned_matrix_scale(matrix: [[f32; 4]; 4]) -> Option<[f32; 3]> {
    const EPSILON: f32 = 1e-5;
    for (column, values) in matrix.iter().enumerate() {
        for (row, value) in values.iter().enumerate() {
            let allowed = column == row || column == 3;
            if !allowed && value.abs() > EPSILON {
                return None;
            }
        }
    }
    if matrix[0][3].abs() > EPSILON
        || matrix[1][3].abs() > EPSILON
        || matrix[2][3].abs() > EPSILON
        || (matrix[3][3] - 1.0).abs() > EPSILON
    {
        return None;
    }
    Some([matrix[0][0], matrix[1][1], matrix[2][2]])
}

fn mesh_position_bounds(mesh: &ImportedMesh) -> Option<([f32; 3], [f32; 3])> {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut any = false;
    for position in mesh
        .primitives
        .iter()
        .flat_map(|primitive| primitive.positions.iter())
    {
        any = true;
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    any.then_some((min, max))
}

fn normalize3(value: [f32; 3]) -> [f32; 3] {
    let length = value
        .iter()
        .map(|component| component * component)
        .sum::<f32>()
        .sqrt();
    if length > 1e-8 {
        [value[0] / length, value[1] / length, value[2] / length]
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn repair_normal(normal: [f32; 3], position_scale: [f32; 3]) -> [f32; 3] {
    normalize3([
        normal[0] / position_scale[0],
        normal[1] / position_scale[1],
        normal[2] / position_scale[2],
    ])
}

fn bind_shape_reflection_axis(
    source_center: [f32; 3],
    target_center: [f32; 3],
    target_extent: [f32; 3],
    inverted_winding: bool,
) -> Option<usize> {
    if !inverted_winding {
        return None;
    }
    let axis = (0..3).max_by(|&a, &b| {
        let a_score = (source_center[a] - target_center[a]).abs() / target_extent[a];
        let b_score = (source_center[b] - target_center[b]).abs() / target_extent[b];
        a_score.total_cmp(&b_score)
    })?;
    ((source_center[axis] - target_center[axis]).abs() / target_extent[axis] > 2.0).then_some(axis)
}

fn has_inverted_winding(primitive: &ImportedPrimitive) -> bool {
    if primitive.normals.len() != primitive.positions.len() {
        return false;
    }
    let mut positive = 0usize;
    let mut negative = 0usize;
    for triangle in primitive.indices.chunks_exact(3) {
        let [ia, ib, ic] = [triangle[0], triangle[1], triangle[2]];
        let Some((a, b, c)) = primitive
            .positions
            .get(ia as usize)
            .zip(primitive.positions.get(ib as usize))
            .zip(primitive.positions.get(ic as usize))
            .map(|((a, b), c)| (*a, *b, *c))
        else {
            continue;
        };
        let edge_ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let edge_ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let face_normal = [
            edge_ab[1] * edge_ac[2] - edge_ab[2] * edge_ac[1],
            edge_ab[2] * edge_ac[0] - edge_ab[0] * edge_ac[2],
            edge_ab[0] * edge_ac[1] - edge_ab[1] * edge_ac[0],
        ];
        let vertex_normal = [
            primitive.normals[ia as usize][0]
                + primitive.normals[ib as usize][0]
                + primitive.normals[ic as usize][0],
            primitive.normals[ia as usize][1]
                + primitive.normals[ib as usize][1]
                + primitive.normals[ic as usize][1],
            primitive.normals[ia as usize][2]
                + primitive.normals[ib as usize][2]
                + primitive.normals[ic as usize][2],
        ];
        let agreement = face_normal[0] * vertex_normal[0]
            + face_normal[1] * vertex_normal[1]
            + face_normal[2] * vertex_normal[2];
        if agreement > 1e-8 {
            positive += 1;
        } else if agreement < -1e-8 {
            negative += 1;
        }
    }
    negative > 0 && negative > positive * 4
}

#[cfg(test)]
mod sketchfab_bind_shape_tests {
    use super::*;

    #[test]
    fn detects_consistently_inverted_triangle_winding() {
        let primitive = ImportedPrimitive {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, -1.0]; 3],
            uv0: Vec::new(),
            indices: vec![0, 1, 2],
            material_texture: None,
            material_alpha_blend: false,
            morph_targets: Vec::new(),
            skin_influences: None,
        };

        assert!(has_inverted_winding(&primitive));
        assert_eq!(
            bind_shape_reflection_axis(
                [0.5625, -25.8125, 0.085],
                [0.125, 2.4583333, 0.019],
                [2.0, 0.6666667, 2.0],
                true,
            ),
            Some(1)
        );
    }

    #[test]
    fn repairs_sketchfab_companion_bind_shape() {
        let mut parent = ImportedNode::identity("parent");
        parent.children = vec![1, 2];

        let mut companion = ImportedNode::identity("companion");
        companion.translation = [1.0, 39.0, 0.16];
        companion.scale = [16.0, 24.0, 8.0];

        let mut mesh_node = ImportedNode::identity("mesh");
        mesh_node.skin = Some(0);
        mesh_node.mesh = Some(ImportedMesh {
            name: "body".to_string(),
            primitives: vec![ImportedPrimitive {
                positions: vec![[-4.0, 1.0, -0.5], [5.0, 2.0, 0.5]],
                normals: vec![[1.0, 1.0, 0.0], [1.0, 1.0, 0.0]],
                uv0: Vec::new(),
                indices: Vec::new(),
                material_texture: None,
                material_alpha_blend: false,
                morph_targets: vec![ImportedMorphTarget {
                    position_deltas: vec![[9.0, 1.0, 1.0], [9.0, 1.0, 1.0]],
                    normal_deltas: None,
                }],
                skin_influences: Some(vec![
                    vec![ImportedJointInfluence {
                        joint: 0,
                        weight: 1.0,
                    }],
                    vec![ImportedJointInfluence {
                        joint: 0,
                        weight: 1.0,
                    }],
                ]),
            }],
        });

        let skin = ImportedSkin {
            joints: vec![0],
            inverse_bind_matrices: vec![[
                [8.0, 0.0, 0.0, 0.0],
                [0.0, 24.0, 0.0, 0.0],
                [0.0, 0.0, 8.0, 0.0],
                [-1.0, -39.0, -0.16, 1.0],
            ]],
        };
        let mut nodes = vec![parent, companion, mesh_node];
        let mut diagnostics = Diagnostics::default();

        repair_sketchfab_bind_shapes(&mut nodes, &[skin], &mut diagnostics);

        let positions = &nodes[2].mesh.as_ref().unwrap().primitives[0].positions;
        assert_eq!(positions, &[[-0.875, 1.125, -0.48], [1.125, 2.125, 0.52]]);
        assert_eq!(
            nodes[2].mesh.as_ref().unwrap().primitives[0].morph_targets[0]
                .position_deltas
                .as_slice(),
            &[[2.0, 1.0, 1.0], [2.0, 1.0, 1.0]]
        );
        assert!(
            diagnostics
                .messages()
                .any(|message| message.contains("repaired nonstandard Sketchfab bind shape"))
        );
    }
}

fn load_mesh(
    mesh: &gltf::Mesh,
    buffer_data: &[Vec<u8>],
    texture_importer: &mut TextureImporter<'_>,
    diagnostics: &mut Diagnostics,
) -> Result<ImportedMesh, ImportError> {
    let name = format!("mesh{}", mesh.index());

    let mut primitives = Vec::with_capacity(mesh.primitives().count());
    for primitive in mesh.primitives() {
        primitives.push(load_primitive(
            &name,
            &primitive,
            buffer_data,
            texture_importer,
            diagnostics,
        )?);
    }

    Ok(ImportedMesh { name, primitives })
}

fn load_primitive(
    mesh_name: &str,
    primitive: &gltf::Primitive,
    buffer_data: &[Vec<u8>],
    texture_importer: &mut TextureImporter<'_>,
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
    let specular_glossiness = material.pbr_specular_glossiness();
    if specular_glossiness.is_some() {
        diagnostics.push(format!(
            "mesh `{mesh_name}` primitive #{} uses KHR_materials_pbrSpecularGlossiness; imported its diffuse texture and alpha mode, and dropped specular/glossiness properties",
            primitive.index()
        ));
    }
    let material_texture = specular_glossiness
        .as_ref()
        .and_then(|material| material.diffuse_texture())
        .or_else(|| material.pbr_metallic_roughness().base_color_texture())
        .map(|info| info.texture().source())
        .map(|image| texture_importer.import(&image, diagnostics))
        .transpose()?;

    let joints0 = reader
        .read_joints(0)
        .map(|joints| joints.into_u16().collect());
    let weights0 = reader
        .read_weights(0)
        .map(|weights| weights.into_f32().collect());
    let joints1 = reader
        .read_joints(1)
        .map(|joints| joints.into_u16().collect());
    let weights1 = reader
        .read_weights(1)
        .map(|weights| weights.into_f32().collect());
    let skin_influences = load_skin_influences(
        mesh_name,
        primitive.index(),
        positions.len(),
        joints0,
        weights0,
        joints1,
        weights1,
    )?;

    let mut morph_targets = Vec::new();
    for (target_index, (positions_iter, normals_iter, _tangents_iter)) in
        reader.read_morph_targets().enumerate()
    {
        let position_deltas: Vec<[f32; 3]> = positions_iter
            .ok_or_else(|| ImportError::MissingAttribute {
                mesh: mesh_name.to_string(),
                primitive: primitive.index(),
                attribute: "morph target POSITION",
            })?
            .collect();
        let normal_deltas: Option<Vec<[f32; 3]>> = normals_iter.map(|it| it.collect());
        if position_deltas.len() != positions.len() {
            return Err(ImportError::MorphTargetAttributeCountMismatch {
                mesh: mesh_name.to_string(),
                primitive: primitive.index(),
                target: target_index,
                attribute: "POSITION",
                expected: positions.len(),
                actual: position_deltas.len(),
            });
        }
        if let Some(normal_deltas) = &normal_deltas {
            if normal_deltas.len() != positions.len() {
                return Err(ImportError::MorphTargetAttributeCountMismatch {
                    mesh: mesh_name.to_string(),
                    primitive: primitive.index(),
                    target: target_index,
                    attribute: "NORMAL",
                    expected: positions.len(),
                    actual: normal_deltas.len(),
                });
            }
        }
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
        skin_influences,
    })
}

#[allow(clippy::too_many_arguments)]
fn load_skin_influences(
    mesh: &str,
    primitive: usize,
    positions: usize,
    joints0: Option<Vec<[u16; 4]>>,
    weights0: Option<Vec<[f32; 4]>>,
    joints1: Option<Vec<[u16; 4]>>,
    weights1: Option<Vec<[f32; 4]>>,
) -> Result<Option<Vec<Vec<ImportedJointInfluence>>>, ImportError> {
    for (set, joints, weights) in [(0, &joints0, &weights0), (1, &joints1, &weights1)] {
        if joints.is_some() != weights.is_some() {
            return Err(ImportError::IncompleteSkinAttributes {
                mesh: mesh.to_string(),
                primitive,
                set,
            });
        }
        if let (Some(joints), Some(weights)) = (joints, weights) {
            if joints.len() != positions || weights.len() != positions {
                return Err(ImportError::SkinAttributeCountMismatch {
                    mesh: mesh.to_string(),
                    primitive,
                    joints: joints.len(),
                    weights: weights.len(),
                    positions,
                });
            }
        }
    }

    let Some(joints0) = joints0 else {
        return Ok(None);
    };
    let weights0 = weights0.expect("presence checked above");
    let mut vertices = Vec::with_capacity(positions);
    for vertex in 0..positions {
        let mut influences = Vec::with_capacity(if joints1.is_some() { 8 } else { 4 });
        for (&joint, &weight) in joints0[vertex].iter().zip(&weights0[vertex]) {
            if weight.is_finite() && weight > 0.0 {
                influences.push(ImportedJointInfluence { joint, weight });
            }
        }
        if let (Some(joints1), Some(weights1)) = (&joints1, &weights1) {
            for (&joint, &weight) in joints1[vertex].iter().zip(&weights1[vertex]) {
                if weight.is_finite() && weight > 0.0 {
                    influences.push(ImportedJointInfluence { joint, weight });
                }
            }
        }
        vertices.push(influences);
    }
    Ok(Some(vertices))
}

fn load_animation(
    animation: &gltf::Animation,
    buffer_data: &[Vec<u8>],
    diagnostics: &mut Diagnostics,
) -> Result<ImportedAnimation, ImportError> {
    let name = format!("animation{}", animation.index());

    let mut weight_channels = Vec::new();
    let mut trs_channels = Vec::new();

    for channel in animation.channels() {
        let target = channel.target();
        let node = target.node();
        let node_name = format!("node{}", node.index());

        let (interpolation, cubic_spline) = match channel.sampler().interpolation() {
            gltf::animation::Interpolation::Linear => (Interpolation::Linear, false),
            gltf::animation::Interpolation::Step => (Interpolation::Step, false),
            gltf::animation::Interpolation::CubicSpline => {
                diagnostics.push(format!(
                    "animation `{name}` channel targeting `{node_name}` uses CUBICSPLINE; imported key values with LINEAR interpolation and dropped tangents"
                ));
                (Interpolation::Linear, true)
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
                let raw: Vec<_> = it.map(|[x, y, z]| [x, y, z, 0.0]).collect();
                let values = collapse_cubic_trs_values(
                    &name,
                    &node_name,
                    "translation",
                    &times,
                    raw,
                    cubic_spline,
                )?;
                validate_animation_sample_count(
                    &name,
                    &node_name,
                    "translation",
                    times.len(),
                    values.len(),
                )?;
                trs_channels.push(ImportedTrsChannel {
                    node: node.index(),
                    property: TrsProperty::Translation,
                    times,
                    values,
                    interpolation,
                });
            }
            ReadOutputs::Scales(it) => {
                let raw: Vec<_> = it.map(|[x, y, z]| [x, y, z, 0.0]).collect();
                let values = collapse_cubic_trs_values(
                    &name,
                    &node_name,
                    "scale",
                    &times,
                    raw,
                    cubic_spline,
                )?;
                validate_animation_sample_count(
                    &name,
                    &node_name,
                    "scale",
                    times.len(),
                    values.len(),
                )?;
                trs_channels.push(ImportedTrsChannel {
                    node: node.index(),
                    property: TrsProperty::Scale,
                    times,
                    values,
                    interpolation,
                });
            }
            ReadOutputs::Rotations(rotations) => {
                let raw: Vec<[f32; 4]> = rotations.into_f32().collect();
                let values = collapse_cubic_trs_values(
                    &name,
                    &node_name,
                    "rotation",
                    &times,
                    raw,
                    cubic_spline,
                )?;
                validate_animation_sample_count(
                    &name,
                    &node_name,
                    "rotation",
                    times.len(),
                    values.len(),
                )?;
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
                        } else if cubic_spline {
                            flat.len() / (times.len() * 3)
                        } else {
                            flat.len() / times.len()
                        }
                    });
                let flat = collapse_cubic_weight_values(
                    &name,
                    &node_name,
                    &times,
                    target_count,
                    flat,
                    cubic_spline,
                )?;
                validate_animation_sample_count(
                    &name,
                    &node_name,
                    "weights",
                    times.len().saturating_mul(target_count),
                    flat.len(),
                )?;
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

fn collapse_cubic_trs_values(
    animation: &str,
    node: &str,
    property: &'static str,
    times: &[f32],
    values: Vec<[f32; 4]>,
    cubic_spline: bool,
) -> Result<Vec<[f32; 4]>, ImportError> {
    if !cubic_spline {
        return Ok(values);
    }
    validate_animation_sample_count(
        animation,
        node,
        property,
        times.len().saturating_mul(3),
        values.len(),
    )?;
    Ok(values
        .chunks_exact(3)
        .map(|tangent_value_tangent| tangent_value_tangent[1])
        .collect())
}

fn collapse_cubic_weight_values(
    animation: &str,
    node: &str,
    times: &[f32],
    target_count: usize,
    values: Vec<f32>,
    cubic_spline: bool,
) -> Result<Vec<f32>, ImportError> {
    if !cubic_spline {
        return Ok(values);
    }
    validate_animation_sample_count(
        animation,
        node,
        "weights",
        times.len().saturating_mul(target_count).saturating_mul(3),
        values.len(),
    )?;
    let mut collapsed = Vec::with_capacity(times.len() * target_count);
    for keyframe in 0..times.len() {
        let value_start = keyframe * target_count * 3 + target_count;
        collapsed.extend_from_slice(&values[value_start..value_start + target_count]);
    }
    Ok(collapsed)
}

fn validate_animation_sample_count(
    animation: &str,
    node: &str,
    property: &'static str,
    inputs: usize,
    outputs: usize,
) -> Result<(), ImportError> {
    if inputs != outputs {
        return Err(ImportError::AnimationSamplerCountMismatch {
            animation: animation.to_string(),
            node: node.to_string(),
            property,
            inputs,
            outputs,
        });
    }
    Ok(())
}

struct TextureImporter<'a> {
    buffer_data: &'a [Vec<u8>],
    base_dir: &'a Path,
    imported_by_image: HashMap<usize, String>,
    used_names: HashSet<String>,
    textures: Vec<ImportedTexture>,
}

impl<'a> TextureImporter<'a> {
    fn new(buffer_data: &'a [Vec<u8>], base_dir: &'a Path) -> Self {
        Self {
            buffer_data,
            base_dir,
            imported_by_image: HashMap::new(),
            used_names: HashSet::new(),
            textures: Vec::new(),
        }
    }

    fn import(
        &mut self,
        image: &gltf::Image,
        diagnostics: &mut Diagnostics,
    ) -> Result<String, ImportError> {
        if let Some(path) = self.imported_by_image.get(&image.index()) {
            return Ok(path.clone());
        }

        let (source_name, encoded) = match image.source() {
            gltf::image::Source::Uri { uri, .. } if is_data_uri(uri) => (
                format!("embedded_image_{}", image.index()),
                decode_data_uri(uri)?,
            ),
            gltf::image::Source::Uri { uri, .. } => {
                if is_remote_uri(uri) {
                    return Err(ImportError::UnsupportedSource(format!(
                        "remote/network URI image: {}",
                        truncate_for_error(uri)
                    )));
                }
                let rel = percent_decode(uri);
                reject_unsafe_relative_path(&rel)?;
                let joined = self.base_dir.join(&rel);
                let safe_path = ensure_within_base_dir(self.base_dir, &joined, uri)?;
                let bytes = std::fs::read(&safe_path).map_err(|source| ImportError::Io {
                    path: safe_path,
                    source,
                })?;
                let stem = Path::new(&rel)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("image_{}", image.index()));
                (stem, bytes)
            }
            gltf::image::Source::View { view, .. } => {
                let data = self
                    .buffer_data
                    .get(view.buffer().index())
                    .and_then(|buffer| {
                        let start = view.offset();
                        let end = start.checked_add(view.length())?;
                        buffer.get(start..end)
                    })
                    .ok_or_else(|| ImportError::ImageDecode {
                        index: image.index(),
                        message: "bufferView is outside its backing buffer".to_string(),
                    })?
                    .to_vec();
                (format!("embedded_image_{}", image.index()), data)
            }
        };

        let decoded = image::load_from_memory(&encoded)
            .or_else(|_| image::load_from_memory_with_format(&encoded, image::ImageFormat::Tga))
            .map_err(|err| ImportError::ImageDecode {
                index: image.index(),
                message: err.to_string(),
            })?;
        let mut output = Cursor::new(Vec::new());
        decoded
            .write_to(&mut output, image::ImageOutputFormat::Tga)
            .map_err(|source| ImportError::ImageEncode {
                index: image.index(),
                source,
            })?;

        let file_stem = sanitize_texture_stem(&source_name, image.index());
        let file_name = self.unique_tga_name(&file_stem);
        let relative_path = format!("_yaobow_import/{file_name}");
        diagnostics.push(format!(
            "converted glTF image #{} to `{relative_path}`",
            image.index()
        ));
        self.textures.push(ImportedTexture {
            image_index: image.index(),
            relative_path: relative_path.clone(),
            bytes: output.into_inner(),
        });
        self.imported_by_image
            .insert(image.index(), relative_path.clone());
        Ok(relative_path)
    }

    fn unique_tga_name(&mut self, stem: &str) -> String {
        let mut suffix = 1usize;
        loop {
            let candidate = if suffix == 1 {
                format!("{stem}.tga")
            } else {
                format!("{stem}_{suffix}.tga")
            };
            if self.used_names.insert(candidate.to_lowercase()) {
                return candidate;
            }
            suffix += 1;
        }
    }
}

fn sanitize_texture_stem(source: &str, image_index: usize) -> String {
    let stem: String = source
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
        .collect();
    let stem = stem.trim().trim_matches('.');
    if stem.is_empty() {
        format!("image_{image_index}")
    } else {
        stem.to_string()
    }
}

fn decode_data_uri(uri: &str) -> Result<Vec<u8>, ImportError> {
    let Some((metadata, payload)) = uri.strip_prefix("data:").and_then(|s| s.split_once(','))
    else {
        return Err(ImportError::UnsupportedSource(format!(
            "malformed image data URI: {}",
            truncate_for_error(uri)
        )));
    };
    if metadata.split(';').any(|part| part == "base64") {
        base64::engine::general_purpose::STANDARD
            .decode(payload)
            .map_err(|err| ImportError::UnsupportedSource(format!("invalid image data URI: {err}")))
    } else {
        Ok(percent_decode_bytes(payload))
    }
}

fn percent_decode_bytes(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
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

    fn one_pixel_png() -> Vec<u8> {
        let mut output = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            1,
            1,
            image::Rgba([10, 20, 30, 128]),
        ))
        .write_to(&mut output, image::ImageOutputFormat::Png)
        .expect("encode PNG");
        output.into_inner()
    }

    fn triangle_with_embedded_texture(base_color_texture: bool) -> (gltf::Gltf, usize, usize) {
        let mut builder = SceneBuilder::new();
        let image = builder.add_image_embedded(&one_pixel_png(), "image/png");
        let texture = builder.add_texture(image);
        let material = builder.add_material(base_color_texture.then_some(texture), true);
        let mesh = builder.add_triangle_mesh(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            None,
            &[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            &[0, 1, 2],
            Some(material),
            &[],
        );
        let node = builder.add_node(Some(mesh), &[], None, None, None);
        (builder.parse(&[node]), material, texture)
    }

    fn gltf_to_glb_with_json(
        gltf: &gltf::Gltf,
        mutate: impl FnOnce(&mut serde_json::Value),
    ) -> Vec<u8> {
        let mut json = serde_json::to_value(gltf.document.as_json()).expect("serialize glTF JSON");
        mutate(&mut json);
        let glb = gltf::binary::Glb {
            header: gltf::binary::Header {
                magic: *b"glTF",
                version: 2,
                length: 0,
            },
            json: std::borrow::Cow::Owned(
                serde_json::to_vec(&json).expect("serialize modified glTF JSON"),
            ),
            bin: gltf.blob.clone().map(std::borrow::Cow::Owned),
        };
        glb.to_vec().expect("assemble GLB")
    }

    #[test]
    fn specular_glossiness_diffuse_texture_maps_to_builtin_material() {
        let base_dir = scratch_dir("specular_glossiness");
        let (gltf, material, texture) = triangle_with_embedded_texture(false);
        let bytes = gltf_to_glb_with_json(&gltf, |json| {
            json["materials"][material]["extensions"] = serde_json::json!({
                "KHR_materials_pbrSpecularGlossiness": {
                    "diffuseTexture": { "index": texture },
                    "specularFactor": [0.8, 0.7, 0.6],
                    "glossinessFactor": 0.9
                }
            });
            json["extensionsUsed"] = serde_json::json!(["KHR_materials_pbrSpecularGlossiness"]);
            json["extensionsRequired"] = serde_json::json!(["KHR_materials_pbrSpecularGlossiness"]);
        });
        let path = base_dir.join("model.glb");
        std::fs::write(&path, bytes).expect("write GLB");

        let (scene, diagnostics) = load_gltf_scene(&path).expect("load should succeed");
        let primitive = &scene.nodes[0].mesh.as_ref().unwrap().primitives[0];
        assert_eq!(
            primitive.material_texture.as_deref(),
            Some("_yaobow_import/embedded_image_0.tga")
        );
        assert!(primitive.material_alpha_blend);
        assert!(
            diagnostics
                .messages()
                .any(|message| message.contains("dropped specular/glossiness properties")),
            "expected lossy material diagnostic: {diagnostics:?}"
        );
    }

    #[test]
    fn unsupported_required_material_extension_is_ignored_lossily() {
        let base_dir = scratch_dir("unsupported_material_extension");
        let (gltf, material, _) = triangle_with_embedded_texture(true);
        let bytes = gltf_to_glb_with_json(&gltf, |json| {
            json["materials"][material]["extensions"] = serde_json::json!({
                "KHR_materials_clearcoat": { "clearcoatFactor": 1.0 }
            });
            json["extensionsUsed"] = serde_json::json!(["KHR_materials_clearcoat"]);
            json["extensionsRequired"] = serde_json::json!(["KHR_materials_clearcoat"]);
        });
        let path = base_dir.join("model.glb");
        std::fs::write(&path, bytes).expect("write GLB");

        let (scene, diagnostics) = load_gltf_scene(&path).expect("load should succeed");
        let primitive = &scene.nodes[0].mesh.as_ref().unwrap().primitives[0];
        assert_eq!(
            primitive.material_texture.as_deref(),
            Some("_yaobow_import/embedded_image_0.tga")
        );
        assert!(
            diagnostics
                .messages()
                .any(|message| message.contains("KHR_materials_clearcoat")),
            "expected ignored-extension diagnostic: {diagnostics:?}"
        );
    }

    #[test]
    fn unsupported_required_non_material_extension_still_fails_validation() {
        let base_dir = scratch_dir("unsupported_geometry_extension");
        let path = base_dir.join("model.gltf");
        let json = serde_json::json!({
            "asset": { "version": "2.0" },
            "extensionsUsed": ["KHR_draco_mesh_compression"],
            "extensionsRequired": ["KHR_draco_mesh_compression"]
        });
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).expect("write glTF");

        let error = load_gltf_scene(&path).unwrap_err();
        assert!(
            matches!(error, ImportError::Gltf(gltf::Error::Validation(_))),
            "expected glTF validation error, got {error:?}"
        );
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
    fn image_uri_nonexistent_relative_path_is_an_error() {
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

        let err = load_gltf_scene_from(&gltf, Path::new(".")).unwrap_err();
        assert!(
            matches!(err, ImportError::Io { .. }),
            "expected missing image I/O error, got {err:?}"
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
