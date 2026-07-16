//! Normalized glTF scene → `.mv3` (animated role/prop model) converter.
//!
//! Mirrors [`crate::exporters::gltf::mv3`] in reverse. That exporter
//! emits one glTF node per `(model, mesh)` pair (so each part can carry
//! its own texture), tagging every such node with a
//! `node.extras.yaobow` `{model_index, mesh_index}` payload. This
//! converter regroups nodes that share the same
//! `model_index` back into a single [`Mv3Model`] (see
//! [`group_mv3_nodes`]); every glTF `Primitive` across a group's nodes
//! becomes one [`Mv3Mesh`], with positions/UVs pooled 1:1 per glTF
//! vertex across the whole group (no cross-primitive dedup — simpler
//! and always correct, if a little larger than a hand-optimized
//! packer). Morph targets become additional per-vertex frame
//! snapshots. Generic glTF node hierarchies, mesh-node transforms, morph
//! weights, and skeletal animation are sampled at their source key times and
//! baked into those same full-vertex snapshots.
//!
//! When a scene's mesh-bearing root nodes don't *all* carry the
//! `model_index`/`mesh_index` tag (a plain, hand-authored glTF, or one
//! authored by an older exporter version), this converter falls back to
//! the original convention instead: one mesh-bearing node = one model,
//! with each of that node's `Primitive`s becoming one [`Mv3Mesh`].
//!
//! MV3 itself has no hierarchy or skeleton: a model is a bag of meshes sharing
//! one vertex-frame pool. The converter flattens the glTF hierarchy into world
//! space and CPU-skins each sampled pose before quantization. Every primitive's
//! vertex count must still fit in a `u16` index
//!   ([`ImportError::TooManyVertices`]), and quantized positions must fit
//!   in `i16` after [`Mv3Options::vertex_scale`]
//!   ([`ImportError::QuantizationOverflow`]).
//!
//! Round-tripping a model exported by [`crate::exporters::gltf::mv3`]
//! recovers the original reserved/"unknown" fields and action table from
//! `asset.extras.yaobow` (see [`crate::importers::extras`]); plain,
//! hand-authored glTF gets sensible zeroed defaults instead.

use std::io::{Seek, Write};

use fileformats::mv3::{
    Mv3ActionDesc, Mv3File, Mv3Frame, Mv3Mesh, Mv3Model, Mv3Texture, Mv3Triangle,
    Mv3UnknownDataInFile, Mv3UnknownDataInMesh, write_mv3,
};
use serde::Deserialize;
use serde_json::Value;

use super::error::{Diagnostics, ImportError};
use super::scene::{
    ImportedJointInfluence, ImportedScene, ImportedTrsChannel, Interpolation, TrsProperty,
};
use super::target::{ImportOptions, Mv3Options};

type Mat4 = [[f32; 4]; 4];
const ROTATION_BAKE_FPS: f32 = 30.0;
const MAX_ROTATION_BAKE_FRAMES: usize = 10_000;

#[derive(Debug, Clone, Copy)]
struct FrameSample {
    animation: Option<usize>,
    time: f32,
    timestamp: u32,
}

#[derive(Debug, Clone)]
struct EvaluatedFrame {
    sample: FrameSample,
    world_matrices: Vec<Mat4>,
    skin_matrices: Vec<Option<Vec<Mat4>>>,
}

#[derive(Debug, Clone)]
struct PooledVertex {
    node: usize,
    position: [f32; 3],
    normal: Option<[f32; 3]>,
    morph_position_deltas: Vec<[f32; 3]>,
    morph_normal_deltas: Vec<Option<[f32; 3]>>,
    skin_influences: Option<Vec<ImportedJointInfluence>>,
}

/// Converts `scene` into an in-memory [`Mv3File`], applying `options.mv3`.
pub fn convert(
    scene: &ImportedScene,
    options: &ImportOptions,
) -> Result<(Mv3File, Diagnostics), ImportError> {
    convert_with_template(scene, options, None)
}

/// Like [`convert`], but falls back to `template`'s opaque metadata
/// (`version`/`action_desc`/`unknown_data`/per-model/per-mesh/per-texture
/// reserved fields) wherever `scene` has no `asset.extras.yaobow`
/// round-trip metadata of its own. `extras`, when present, always takes
/// precedence over `template`.
pub fn convert_with_template(
    scene: &ImportedScene,
    options: &ImportOptions,
    template: Option<&Mv3File>,
) -> Result<(Mv3File, Diagnostics), ImportError> {
    let opts = &options.mv3;
    let mut diagnostics = Diagnostics::default();
    let mut extras = mv3_extras(scene, &mut diagnostics);
    if extras.is_none() {
        if let Some(template) = template {
            diagnostics.push(
                "no asset.extras.yaobow round-trip metadata found; falling back to the \
                 replacement template's opaque metadata"
                    .to_string(),
            );
            extras = Some(Mv3Extras::from_file(template));
        }
    }

    let mesh_node_indices: Vec<usize> = scene
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.mesh.is_some())
        .map(|(index, _)| index)
        .collect();
    let groups = group_mv3_nodes(scene, &mesh_node_indices);
    let frame_samples = build_frame_samples(scene, opts, &mut diagnostics);
    let parent_indices = build_parent_indices(scene)?;
    let evaluated_frames = frame_samples
        .iter()
        .map(|sample| {
            let world_matrices = evaluate_world_matrices(scene, &parent_indices, *sample)?;
            let skin_matrices = build_skin_matrices(scene, &world_matrices)?;
            Ok(EvaluatedFrame {
                sample: *sample,
                world_matrices,
                skin_matrices,
            })
        })
        .collect::<Result<Vec<_>, ImportError>>()?;

    let mut textures = Vec::new();
    let mut models = Vec::new();
    for node_indices in &groups {
        let model_metadata = extras
            .as_ref()
            .and_then(|e| e.model_metadata.get(models.len()));
        let (model, texture_name) = build_model(
            scene,
            node_indices,
            opts,
            &evaluated_frames,
            &mut diagnostics,
            model_metadata,
        )?;
        let imported_texture = texture_name
            .as_deref()
            .is_some_and(|name| name.starts_with("_yaobow_import/"));

        let mut texture = Mv3Texture {
            unknown: vec![0.0; 17],
            // GBK-encode (not UTF-8) each name: `Mv3Texture::names` is
            // read back with `SizedString::to_string` (GBK), so a raw
            // UTF-8-encoded Chinese name would come back corrupted.
            names: [
                texture_name.unwrap_or_default(),
                String::new(),
                String::new(),
                String::new(),
            ]
            .iter()
            .map(|n| super::gbk_sized_string(n))
            .collect::<Result<Vec<_>, _>>()?,
        };
        if let Some(extras) = &extras {
            if let Some(t) = extras.textures.get(models.len()) {
                if t.unknown.len() == 17 {
                    texture.unknown = t.unknown.clone();
                }
                if t.names.len() == 4 {
                    let start = usize::from(imported_texture);
                    for (index, name) in t.names.iter().enumerate().skip(start) {
                        texture.names[index] = super::gbk_sized_string(name)?;
                    }
                }
            }
        }
        textures.push(texture);
        models.push(model);
    }

    if models.is_empty() {
        return Err(ImportError::NoGeometry("mv3"));
    }

    let duration = models
        .iter()
        .filter_map(|m: &Mv3Model| m.frames.iter().map(|f| f.timestamp).max())
        .max()
        .unwrap_or(0);

    let action_desc = build_action_desc(scene, &extras, &mut diagnostics)?;
    let unknown_data: Vec<Mv3UnknownDataInFile> = extras
        .as_ref()
        .map(|e| {
            e.file_unknown_data
                .iter()
                .cloned()
                .map(Into::into)
                .collect()
        })
        .unwrap_or_default();

    let file = Mv3File {
        version: extras.as_ref().map(|e| e.version).unwrap_or(4),
        duration,
        texture_count: textures.len() as u32,
        unknown_data_count: unknown_data.len() as u32,
        model_count: models.len() as u32,
        action_count: action_desc.len() as u32,
        action_desc,
        unknown_data,
        textures,
        models,
    };

    Ok((file, diagnostics))
}

/// Converts `scene` and writes the resulting `.mv3` bytes to `writer`.
pub fn write(
    scene: &ImportedScene,
    options: &ImportOptions,
    writer: &mut (impl Write + Seek),
) -> Result<Diagnostics, ImportError> {
    let (file, diagnostics) = convert(scene, options)?;
    write_mv3(writer, &file)?;
    Ok(diagnostics)
}

/// A node's parsed `node.extras.yaobow.payload` MV3 grouping tag (schema
/// 1), as emitted by [`crate::exporters::gltf::mv3::mv3_node_extras`].
#[derive(Debug, Clone, Copy)]
struct Mv3NodeTag {
    model_index: usize,
    mesh_index: usize,
}

/// Parses `node.extras.yaobow` looking for the `{model_index,
/// mesh_index}` payload the exporter tags every mesh node with. Returns
/// `None` for absent/malformed/unrecognized-schema extras — callers
/// treat that as "this node has no round-trip grouping metadata" rather
/// than an error, since a plain hand-authored glTF simply won't have it.
fn mv3_node_tag(extras: Option<&Value>) -> Option<Mv3NodeTag> {
    let envelope = extras?.get("yaobow")?;
    let schema = envelope.get("schema").and_then(Value::as_u64).unwrap_or(1);
    if schema != 1 {
        return None;
    }
    let payload = envelope.get("payload")?;
    Some(Mv3NodeTag {
        model_index: payload.get("model_index")?.as_u64()? as usize,
        mesh_index: payload.get("mesh_index")?.as_u64()? as usize,
    })
}

/// Groups `mesh_node_indices` (every mesh-bearing node) into
/// per-[`Mv3Model`] node-index lists, one inner `Vec` per reconstructed
/// model, ordered by `model_index` (or by first appearance, in the
/// fallback case below); each inner `Vec` is ordered by `mesh_index`.
///
/// [`crate::exporters::gltf::mv3`] emits one glTF node per `(model,
/// mesh)` pair — so a single mv3 model with multiple meshes becomes
/// several *sibling* nodes, not one node with multiple primitives — and
/// tags every such node with a `node.extras.yaobow` `{model_index,
/// mesh_index}` payload recording where it came from. When *every*
/// mesh-bearing root node carries that tag, this groups nodes by
/// `model_index` and orders each group by `mesh_index`, exactly
/// reconstructing the original model/mesh layout regardless of how many
/// separate nodes the exporter split a model's meshes across.
///
/// If any node is missing the tag (a plain, hand-authored glTF, or one
/// produced by a different tool), grouping falls back to the original,
/// simpler convention: one mesh-bearing node = one model, with that node's
/// own `Primitive`s each becoming one [`Mv3Mesh`] (handled by
/// [`build_model`] iterating every node's primitives either way).
fn group_mv3_nodes(scene: &ImportedScene, mesh_node_indices: &[usize]) -> Vec<Vec<usize>> {
    let tags: Vec<Option<Mv3NodeTag>> = mesh_node_indices
        .iter()
        .map(|&index| mv3_node_tag(scene.nodes[index].extras.as_ref()))
        .collect();

    if !mesh_node_indices.is_empty() && tags.iter().all(Option::is_some) {
        let mut by_model: Vec<(usize, Vec<(usize, usize)>)> = Vec::new();
        for (&node_index, tag) in mesh_node_indices.iter().zip(tags.iter()) {
            let tag = tag.expect("checked all() above");
            match by_model.iter_mut().find(|(m, _)| *m == tag.model_index) {
                Some((_, meshes)) => meshes.push((tag.mesh_index, node_index)),
                None => by_model.push((tag.model_index, vec![(tag.mesh_index, node_index)])),
            }
        }
        by_model.sort_by_key(|(model_index, _)| *model_index);
        by_model
            .into_iter()
            .map(|(_, mut meshes)| {
                meshes.sort_by_key(|(mesh_index, _)| *mesh_index);
                meshes
                    .into_iter()
                    .map(|(_, node_index)| node_index)
                    .collect()
            })
            .collect()
    } else {
        mesh_node_indices.iter().map(|&index| vec![index]).collect()
    }
}

/// Builds one [`Mv3Model`] from `node_indices` (one or more
/// nodes belonging to the same original model — see [`group_mv3_nodes`]).
/// Every primitive of every node in the group becomes one [`Mv3Mesh`];
/// positions/UVs/morph deltas are pooled into one shared per-model
/// vertex frame pool (MV3 triangle indices reference that pool
/// directly, not a per-mesh-local one).
fn build_model(
    scene: &ImportedScene,
    node_indices: &[usize],
    opts: &Mv3Options,
    evaluated_frames: &[EvaluatedFrame],
    diagnostics: &mut Diagnostics,
    model_metadata: Option<&Mv3ExtrasModel>,
) -> Result<(Mv3Model, Option<String>), ImportError> {
    let model_name = scene.nodes[node_indices[0]].name.clone();
    let mut pooled_vertices: Vec<PooledVertex> = Vec::new();
    let mut pooled_uvs: Vec<[f32; 2]> = Vec::new();

    let mut meshes = Vec::new();
    let mut texture_name = None;
    for &node_index in node_indices {
        let node = &scene.nodes[node_index];
        let mesh = node.mesh.as_ref().expect("caller checked mesh.is_some()");

        for (prim_index, primitive) in mesh.primitives.iter().enumerate() {
            if primitive.material_texture.is_some() && texture_name.is_none() {
                texture_name = primitive.material_texture.clone();
            }

            let vertex_count = primitive.positions.len();
            if vertex_count > u16::MAX as usize + 1 {
                return Err(ImportError::TooManyVertices {
                    mesh: node.name.clone(),
                    primitive: prim_index,
                    count: vertex_count,
                });
            }
            if node.skin.is_some() && primitive.skin_influences.is_none() {
                return Err(ImportError::MissingSkinAttributes {
                    node: node.name.clone(),
                    primitive: prim_index,
                });
            }

            let base_index = pooled_vertices.len() as u32;
            for i in 0..vertex_count {
                pooled_vertices.push(PooledVertex {
                    node: node_index,
                    position: primitive.positions[i],
                    normal: primitive.normals.get(i).copied(),
                    morph_position_deltas: primitive
                        .morph_targets
                        .iter()
                        .map(|target| target.position_deltas.get(i).copied().unwrap_or([0.0; 3]))
                        .collect(),
                    morph_normal_deltas: primitive
                        .morph_targets
                        .iter()
                        .map(|target| {
                            target
                                .normal_deltas
                                .as_ref()
                                .and_then(|deltas| deltas.get(i).copied())
                        })
                        .collect(),
                    skin_influences: primitive
                        .skin_influences
                        .as_ref()
                        .and_then(|vertices| vertices.get(i).cloned()),
                });
                pooled_uvs.push(primitive.uv0.get(i).copied().unwrap_or([0.0, 0.0]));
            }

            if primitive.indices.len() % 3 != 0 {
                return Err(ImportError::Other(format!(
                    "mesh `{}` primitive #{prim_index} has {} indices, not a multiple of 3",
                    node.name,
                    primitive.indices.len()
                )));
            }
            let mut triangles = Vec::with_capacity(primitive.indices.len() / 3);
            for tri in primitive.indices.chunks_exact(3) {
                let mut idx = [0u16; 3];
                for (slot, &raw) in idx.iter_mut().zip(tri) {
                    let local = raw.checked_add(base_index).ok_or_else(|| {
                        ImportError::IndexRemapOverflow {
                            mesh: node.name.clone(),
                            primitive: prim_index,
                            index: raw,
                            base_index,
                        }
                    })?;
                    if local > u16::MAX as u32 {
                        return Err(ImportError::IndexOutOfRange {
                            mesh: node.name.clone(),
                            primitive: prim_index,
                            index: local,
                            vertex_count: pooled_vertices.len(),
                        });
                    }
                    *slot = local as u16;
                }
                triangles.push(Mv3Triangle {
                    indices: idx,
                    texcoord_indices: idx,
                });
            }

            let mesh_index = meshes.len();
            let mesh_extras = model_metadata.and_then(|m| m.mesh_metadata.get(mesh_index));
            meshes.push(Mv3Mesh {
                unknown: mesh_extras.map(|m| m.unknown).unwrap_or(0),
                triangle_count: triangles.len() as u32,
                triangles,
                unknown_data_count: mesh_extras
                    .map(|m| m.unknown_data.len() as u32)
                    .unwrap_or(0),
                unknown_data: mesh_extras
                    .map(|m| m.unknown_data.iter().map(|d| (*d).into()).collect())
                    .unwrap_or_default(),
            });
        }
    }

    let vertex_per_frame = pooled_vertices.len() as u32;
    if pooled_vertices.is_empty() {
        diagnostics.push(format!(
            "model `{}` produced no vertices; emitting an empty model",
            model_name
        ));
    }

    let mut aabb_min = if pooled_vertices.is_empty() {
        [0.0; 3]
    } else {
        [f32::INFINITY; 3]
    };
    let mut aabb_max = if pooled_vertices.is_empty() {
        [0.0; 3]
    } else {
        [f32::NEG_INFINITY; 3]
    };
    let mut frames = Vec::with_capacity(evaluated_frames.len());
    for frame in evaluated_frames {
        let mut vertices = Vec::with_capacity(pooled_vertices.len());
        for pooled in &pooled_vertices {
            let node = &scene.nodes[pooled.node];
            let (p, normal) = bake_vertex(scene, pooled, frame)?;
            for axis in 0..3 {
                aabb_min[axis] = aabb_min[axis].min(p[axis]);
                aabb_max[axis] = aabb_max[axis].max(p[axis]);
            }
            let (x, y, z) = quantize_i16(p, opts.vertex_scale, &node.name)?;
            let (normal_phi, normal_theta) = encode_normal(normal);
            vertices.push(fileformats::mv3::Mv3Vertex {
                x,
                y,
                z,
                normal_phi,
                normal_theta,
            });
        }
        frames.push(Mv3Frame {
            timestamp: frame.sample.timestamp,
            vertices,
        });
    }

    let model_unknown = model_metadata
        .filter(|m| m.unknown.len() == 64)
        .map(|m| m.unknown.clone())
        .unwrap_or_else(|| vec![0u8; 64]);

    let model = Mv3Model {
        unknown: model_unknown,
        vertex_per_frame,
        aabb_min,
        aabb_max,
        frame_count: evaluated_frames.len() as u32,
        frames,
        texcoord_count: pooled_uvs.len() as u32,
        // MV3 stores UVs authored with an OpenGL-style bottom-left origin
        // (see `exporters::gltf::mv3`'s `1 - v` flip on the way *out*);
        // flip back on the way in so re-exporting round-trips.
        texcoords: pooled_uvs
            .iter()
            .map(|uv| fileformats::rwbs::TexCoord {
                u: uv[0],
                v: 1.0 - uv[1],
            })
            .collect(),
        mesh_count: meshes.len() as u32,
        meshes,
    };

    Ok((model, texture_name))
}

fn build_frame_samples(
    scene: &ImportedScene,
    opts: &Mv3Options,
    diagnostics: &mut Diagnostics,
) -> Vec<FrameSample> {
    if scene.animations.is_empty() {
        return vec![FrameSample {
            animation: None,
            time: 0.0,
            timestamp: 0,
        }];
    }

    if scene.animations.len() > 1 {
        diagnostics.push(format!(
            "source glTF has {} animations; concatenated them into one mv3 vertex-frame timeline",
            scene.animations.len()
        ));
    }

    let mut samples = Vec::new();
    let mut next_clip_tick = 0u32;
    for (animation_index, animation) in scene.animations.iter().enumerate() {
        let mut times = vec![0.0f32];
        times.extend(
            animation
                .trs_channels
                .iter()
                .flat_map(|channel| channel.times.iter().copied()),
        );
        times.extend(
            animation
                .weight_channels
                .iter()
                .flat_map(|channel| channel.times.iter().copied()),
        );
        times.retain(|time| time.is_finite() && *time >= 0.0);
        if animation
            .trs_channels
            .iter()
            .any(|channel| channel.property == TrsProperty::Rotation)
        {
            let duration = times.iter().copied().fold(0.0f32, f32::max);
            let sample_count = (duration * ROTATION_BAKE_FPS).ceil() as usize;
            if sample_count <= MAX_ROTATION_BAKE_FRAMES {
                times.extend(
                    (0..=sample_count)
                        .map(|sample| (sample as f32 / ROTATION_BAKE_FPS).min(duration)),
                );
            } else {
                diagnostics.push(format!(
                    "animation `{}` is too long to resample rotations at {} fps; using source key times only",
                    animation.name, ROTATION_BAKE_FPS
                ));
            }
        }
        times.sort_by(f32::total_cmp);
        times.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

        let mut last_tick = samples.last().map(|sample: &FrameSample| sample.timestamp);
        for time in times {
            let relative_tick = (time * opts.ticks_per_second)
                .round()
                .clamp(0.0, u32::MAX as f32) as u32;
            let mut timestamp = next_clip_tick.saturating_add(relative_tick);
            if let Some(previous) = last_tick {
                if timestamp <= previous {
                    timestamp = previous.saturating_add(1);
                }
            }
            samples.push(FrameSample {
                animation: Some(animation_index),
                time,
                timestamp,
            });
            last_tick = Some(timestamp);
        }
        next_clip_tick = last_tick.unwrap_or(next_clip_tick).saturating_add(1);
    }
    samples
}

fn build_parent_indices(scene: &ImportedScene) -> Result<Vec<Option<usize>>, ImportError> {
    let mut parents: Vec<Option<usize>> = vec![None; scene.nodes.len()];
    for (parent, node) in scene.nodes.iter().enumerate() {
        for &child in &node.children {
            let Some(slot) = parents.get_mut(child) else {
                return Err(ImportError::Other(format!(
                    "node `{}` references missing child node #{}",
                    node.name, child
                )));
            };
            if let Some(first_parent) = *slot {
                return Err(ImportError::MultipleNodeParents {
                    node: scene.nodes[child].name.clone(),
                    first_parent: scene.nodes[first_parent].name.clone(),
                    second_parent: scene.nodes[parent].name.clone(),
                });
            }
            *slot = Some(parent);
        }
    }
    Ok(parents)
}

fn evaluate_world_matrices(
    scene: &ImportedScene,
    parents: &[Option<usize>],
    sample: FrameSample,
) -> Result<Vec<Mat4>, ImportError> {
    let mut matrices = vec![None; scene.nodes.len()];
    let mut state = vec![0u8; scene.nodes.len()];
    for start in 0..scene.nodes.len() {
        if matrices[start].is_some() {
            continue;
        }
        let mut chain = Vec::new();
        let mut current = Some(start);
        while let Some(node_index) = current {
            if matrices[node_index].is_some() {
                break;
            }
            if state[node_index] == 1 {
                return Err(ImportError::NodeHierarchyCycle {
                    node: scene.nodes[node_index].name.clone(),
                });
            }
            state[node_index] = 1;
            chain.push(node_index);
            current = parents[node_index];
        }
        for &node_index in chain.iter().rev() {
            let local = local_matrix(scene, node_index, sample);
            let world = match parents[node_index] {
                Some(parent) => matrix_mul(
                    matrices[parent].expect("parent evaluated before child"),
                    local,
                ),
                None => local,
            };
            matrices[node_index] = Some(world);
            state[node_index] = 2;
        }
    }
    Ok(matrices
        .into_iter()
        .map(|matrix| matrix.expect("all nodes evaluated"))
        .collect())
}

fn local_matrix(scene: &ImportedScene, node_index: usize, sample: FrameSample) -> Mat4 {
    let node = &scene.nodes[node_index];
    let mut translation = node.translation;
    let mut rotation = node.rotation;
    let mut scale = node.scale;

    if let Some(animation_index) = sample.animation {
        let animation = &scene.animations[animation_index];
        for channel in animation
            .trs_channels
            .iter()
            .filter(|channel| channel.node == node_index)
        {
            let value = sample_trs_channel(channel, sample.time);
            match channel.property {
                TrsProperty::Translation => translation = [value[0], value[1], value[2]],
                TrsProperty::Rotation => rotation = value,
                TrsProperty::Scale => scale = [value[0], value[1], value[2]],
            }
        }
    }

    trs_matrix(translation, rotation, scale)
}

fn build_skin_matrices(
    scene: &ImportedScene,
    world_matrices: &[Mat4],
) -> Result<Vec<Option<Vec<Mat4>>>, ImportError> {
    let mut result = vec![None; scene.nodes.len()];
    for (node_index, node) in scene.nodes.iter().enumerate() {
        let Some(skin_index) = node.skin else {
            continue;
        };
        let skin = scene.skins.get(skin_index).ok_or_else(|| {
            ImportError::Other(format!(
                "mesh node `{}` references missing glTF skin #{}",
                node.name, skin_index
            ))
        })?;
        let mut matrices = Vec::with_capacity(skin.joints.len());
        for (&joint_node, inverse_bind) in skin.joints.iter().zip(&skin.inverse_bind_matrices) {
            let joint_world = world_matrices.get(joint_node).ok_or_else(|| {
                ImportError::Other(format!(
                    "glTF skin #{skin_index} references missing joint node #{joint_node}"
                ))
            })?;
            matrices.push(matrix_mul(*joint_world, *inverse_bind));
        }
        result[node_index] = Some(matrices);
    }
    Ok(result)
}

fn bake_vertex(
    scene: &ImportedScene,
    vertex: &PooledVertex,
    frame: &EvaluatedFrame,
) -> Result<([f32; 3], [f32; 3]), ImportError> {
    let morph_weights = sample_morph_weights(
        scene,
        vertex.node,
        vertex.morph_position_deltas.len(),
        frame.sample,
    );
    let mut position = vertex.position;
    let mut normal = vertex.normal.unwrap_or([0.0, 1.0, 0.0]);
    for (target, weight) in vertex
        .morph_position_deltas
        .iter()
        .zip(morph_weights.iter().copied())
    {
        position[0] += target[0] * weight;
        position[1] += target[1] * weight;
        position[2] += target[2] * weight;
    }
    for (target, weight) in vertex
        .morph_normal_deltas
        .iter()
        .zip(morph_weights.iter().copied())
    {
        if let Some(target) = target {
            normal[0] += target[0] * weight;
            normal[1] += target[1] * weight;
            normal[2] += target[2] * weight;
        }
    }

    let node = &scene.nodes[vertex.node];
    let (world_position, world_normal) = match node.skin {
        Some(skin_index) => {
            let skin = &scene.skins[skin_index];
            let skin_matrices = frame.skin_matrices[vertex.node]
                .as_ref()
                .expect("skin matrices built for skinned node");
            let influences = vertex
                .skin_influences
                .as_deref()
                .expect("skinned primitive checked while pooling");
            let total_weight: f32 = influences.iter().map(|influence| influence.weight).sum();
            if influences.is_empty() || total_weight <= 1e-8 {
                (position, normal)
            } else {
                let mut skinned_position = [0.0; 3];
                let mut skinned_normal = [0.0; 3];
                for influence in influences {
                    let matrix = skin_matrices.get(influence.joint as usize).ok_or_else(|| {
                        ImportError::SkinJointOutOfRange {
                            node: node.name.clone(),
                            skin: skin_index,
                            joint: influence.joint,
                            joint_count: skin.joints.len(),
                        }
                    })?;
                    let weight = influence.weight / total_weight;
                    let p = transform_point(*matrix, position);
                    let n = transform_vector(*matrix, normal);
                    for axis in 0..3 {
                        skinned_position[axis] += p[axis] * weight;
                        skinned_normal[axis] += n[axis] * weight;
                    }
                }
                (skinned_position, skinned_normal)
            }
        }
        None => (
            transform_point(frame.world_matrices[vertex.node], position),
            transform_vector(frame.world_matrices[vertex.node], normal),
        ),
    };
    Ok((world_position, normalize3(world_normal)))
}

fn sample_morph_weights(
    scene: &ImportedScene,
    node_index: usize,
    target_count: usize,
    sample: FrameSample,
) -> Vec<f32> {
    let mut weights = vec![0.0; target_count];
    for (target, &weight) in weights
        .iter_mut()
        .zip(scene.nodes[node_index].morph_weights.iter())
    {
        *target = weight;
    }
    let Some(animation_index) = sample.animation else {
        return weights;
    };
    let Some(channel) = scene.animations[animation_index]
        .weight_channels
        .iter()
        .find(|channel| channel.node == node_index)
    else {
        return weights;
    };
    let sampled = sample_weight_channel(channel, sample.time);
    for (target, weight) in weights.iter_mut().zip(sampled) {
        *target = weight;
    }
    weights
}

fn sample_weight_channel(channel: &super::scene::ImportedWeightsChannel, time: f32) -> Vec<f32> {
    let frame_count = channel.times.len();
    if frame_count == 0 || channel.target_count == 0 {
        return Vec::new();
    }
    let (from, to, factor) = sample_segment(&channel.times, channel.interpolation, time);
    (0..channel.target_count)
        .map(|target| {
            let a = channel.weights[from * channel.target_count + target];
            let b = channel.weights[to * channel.target_count + target];
            a + (b - a) * factor
        })
        .collect()
}

fn sample_trs_channel(channel: &ImportedTrsChannel, time: f32) -> [f32; 4] {
    if channel.times.is_empty() {
        return [0.0; 4];
    }
    let (from, to, factor) = sample_segment(&channel.times, channel.interpolation, time);
    let a = channel.values[from];
    let b = channel.values[to];
    if channel.property == TrsProperty::Rotation {
        quaternion_slerp(a, b, factor)
    } else {
        [
            a[0] + (b[0] - a[0]) * factor,
            a[1] + (b[1] - a[1]) * factor,
            a[2] + (b[2] - a[2]) * factor,
            a[3] + (b[3] - a[3]) * factor,
        ]
    }
}

fn sample_segment(times: &[f32], interpolation: Interpolation, time: f32) -> (usize, usize, f32) {
    if times.len() <= 1 || time <= times[0] {
        return (0, 0, 0.0);
    }
    let last = times.len() - 1;
    if time >= times[last] {
        return (last, last, 0.0);
    }
    let to = times
        .iter()
        .position(|sample_time| *sample_time > time)
        .unwrap_or(last);
    let from = to - 1;
    if interpolation == Interpolation::Step {
        return (from, from, 0.0);
    }
    let span = times[to] - times[from];
    let factor = if span > 1e-8 {
        (time - times[from]) / span
    } else {
        0.0
    };
    (from, to, factor)
}

fn trs_matrix(translation: [f32; 3], rotation: [f32; 4], scale: [f32; 3]) -> Mat4 {
    let [x, y, z, w] = normalize4(rotation);
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let xw = x * w;
    let yw = y * w;
    let zw = z * w;
    [
        [
            (1.0 - 2.0 * (yy + zz)) * scale[0],
            (2.0 * (xy + zw)) * scale[0],
            (2.0 * (xz - yw)) * scale[0],
            0.0,
        ],
        [
            (2.0 * (xy - zw)) * scale[1],
            (1.0 - 2.0 * (xx + zz)) * scale[1],
            (2.0 * (yz + xw)) * scale[1],
            0.0,
        ],
        [
            (2.0 * (xz + yw)) * scale[2],
            (2.0 * (yz - xw)) * scale[2],
            (1.0 - 2.0 * (xx + yy)) * scale[2],
            0.0,
        ],
        [translation[0], translation[1], translation[2], 1.0],
    ]
}

fn matrix_mul(a: Mat4, b: Mat4) -> Mat4 {
    let mut out = [[0.0; 4]; 4];
    for column in 0..4 {
        for row in 0..4 {
            out[column][row] = (0..4).map(|k| a[k][row] * b[column][k]).sum();
        }
    }
    out
}

fn transform_point(matrix: Mat4, point: [f32; 3]) -> [f32; 3] {
    [
        matrix[0][0] * point[0] + matrix[1][0] * point[1] + matrix[2][0] * point[2] + matrix[3][0],
        matrix[0][1] * point[0] + matrix[1][1] * point[1] + matrix[2][1] * point[2] + matrix[3][1],
        matrix[0][2] * point[0] + matrix[1][2] * point[1] + matrix[2][2] * point[2] + matrix[3][2],
    ]
}

fn transform_vector(matrix: Mat4, vector: [f32; 3]) -> [f32; 3] {
    [
        matrix[0][0] * vector[0] + matrix[1][0] * vector[1] + matrix[2][0] * vector[2],
        matrix[0][1] * vector[0] + matrix[1][1] * vector[1] + matrix[2][1] * vector[2],
        matrix[0][2] * vector[0] + matrix[1][2] * vector[1] + matrix[2][2] * vector[2],
    ]
}

fn normalize3(value: [f32; 3]) -> [f32; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if length > 1e-8 {
        [value[0] / length, value[1] / length, value[2] / length]
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn normalize4(value: [f32; 4]) -> [f32; 4] {
    let length = value
        .iter()
        .map(|component| component * component)
        .sum::<f32>()
        .sqrt();
    if length > 1e-8 {
        [
            value[0] / length,
            value[1] / length,
            value[2] / length,
            value[3] / length,
        ]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

fn quaternion_slerp(a: [f32; 4], mut b: [f32; 4], factor: f32) -> [f32; 4] {
    let a = normalize4(a);
    b = normalize4(b);
    let mut dot = a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>();
    if dot < 0.0 {
        b = [-b[0], -b[1], -b[2], -b[3]];
        dot = -dot;
    }
    if dot > 0.9995 {
        return normalize4([
            a[0] + (b[0] - a[0]) * factor,
            a[1] + (b[1] - a[1]) * factor,
            a[2] + (b[2] - a[2]) * factor,
            a[3] + (b[3] - a[3]) * factor,
        ]);
    }
    let angle = dot.clamp(-1.0, 1.0).acos();
    let sin_angle = angle.sin();
    let from_weight = ((1.0 - factor) * angle).sin() / sin_angle;
    let to_weight = (factor * angle).sin() / sin_angle;
    normalize4([
        a[0] * from_weight + b[0] * to_weight,
        a[1] * from_weight + b[1] * to_weight,
        a[2] * from_weight + b[2] * to_weight,
        a[3] * from_weight + b[3] * to_weight,
    ])
}

fn quantize_i16(
    p: [f32; 3],
    vertex_scale: f32,
    mesh: &str,
) -> Result<(i16, i16, i16), ImportError> {
    let names = ["x", "y", "z"];
    let mut out = [0i16; 3];
    for i in 0..3 {
        let q = (p[i] / vertex_scale).round();
        if q < i16::MIN as f32 || q > i16::MAX as f32 {
            return Err(ImportError::QuantizationOverflow {
                mesh: mesh.to_string(),
                component: names[i],
                value: p[i],
                scale: vertex_scale,
                quantized: q as f64,
            });
        }
        out[i] = q as i16;
    }
    Ok((out[0], out[1], out[2]))
}

/// Approximate spherical (phi/theta) encoding of a normal. **Not**
/// load-bearing: the engine recomputes smooth normals from geometry at
/// load time and never decodes this field (see
/// `openpal3::scene::role_controller::create_geometry_frames`), so this
/// only needs to be a plausible value for other tooling.
fn encode_normal(n: [f32; 3]) -> (i8, u8) {
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    let (nx, ny, nz) = if len > 1e-6 {
        (n[0] / len, n[1] / len, n[2] / len)
    } else {
        (0.0, 1.0, 0.0)
    };
    let theta = ny.clamp(-1.0, 1.0).acos();
    let phi = nz.atan2(nx);
    let theta_u8 = (theta / std::f32::consts::PI * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let phi_i8 = (phi / std::f32::consts::PI * 127.0)
        .round()
        .clamp(-127.0, 127.0) as i8;
    (phi_i8, theta_u8)
}

fn build_action_desc(
    scene: &ImportedScene,
    extras: &Option<Mv3Extras>,
    diagnostics: &mut Diagnostics,
) -> Result<Vec<Mv3ActionDesc>, ImportError> {
    if let Some(extras) = extras {
        if !extras.action_desc.is_empty() {
            let mut out = Vec::with_capacity(extras.action_desc.len());
            for a in &extras.action_desc {
                // GBK-encode (not UTF-8) before the capacity check/
                // construction: `Mv3ActionDesc::name` is read back with
                // `StringWithCapacity::as_str` (GBK), so a raw-UTF-8-
                // encoded Chinese name would both mis-measure the
                // 16-byte capacity check and come back corrupted on
                // read.
                let name = super::gbk_capacity_string(&a.name, 16, |actual, limit| {
                    ImportError::NameTooLong {
                        mesh: "<mv3 action table>".to_string(),
                        name: a.name.clone(),
                        actual,
                        limit,
                    }
                })?;
                out.push(Mv3ActionDesc { tick: a.tick, name });
            }
            return Ok(out);
        }
    }

    // No round-trip metadata to recover real action names from: the
    // `gltf` crate only exposes node/mesh/animation `name()` accessors
    // behind its `names` feature, which this importer deliberately does
    // not enable (see `importers::loader` module docs — enabling it would
    // force a `name` field into `gltf-json` structs that the
    // concurrently-developed `exporters::gltf` crate code doesn't
    // populate). So a plain, non-round-tripped glTF has no action names
    // to reconstruct; emit no action table rather than a synthetic one
    // (an empty `action_desc` is valid MV3).
    if !scene.animations.is_empty() {
        diagnostics.push(
            "source glTF has animations but no asset.extras.yaobow round-trip metadata; \
             emitting no mv3 action table since original action names aren't recoverable"
                .to_string(),
        );
    }
    Ok(Vec::new())
}

#[derive(Debug, Default, Deserialize)]
struct Mv3ExtrasTexture {
    #[serde(default)]
    unknown: Vec<f32>,
    #[serde(default)]
    names: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Mv3ExtrasAction {
    tick: u32,
    name: String,
}

/// Mirrors [`fileformats::mv3::Mv3UnknownDataInFile`] field-for-field.
/// `Deserialize` can't be derived directly on the `fileformats` type
/// (only `Serialize` is, matching what the exporter needs to write
/// `asset.extras.yaobow`), so this shadow struct exists purely to parse
/// the JSON back before converting into the real type below.
#[derive(Debug, Default, Clone, Deserialize)]
struct Mv3ExtrasUnknownDataInFile {
    #[serde(default)]
    unknown0: Vec<u8>,
    #[serde(default)]
    unknown1: u32,
    #[serde(default)]
    unknown2_count: u32,
    #[serde(default)]
    unknown2: Vec<[f32; 17]>,
}

impl From<Mv3ExtrasUnknownDataInFile> for Mv3UnknownDataInFile {
    fn from(v: Mv3ExtrasUnknownDataInFile) -> Self {
        Mv3UnknownDataInFile {
            unknown0: v.unknown0,
            unknown1: v.unknown1,
            unknown2_count: v.unknown2_count,
            unknown2: v.unknown2,
        }
    }
}

/// Mirrors [`fileformats::mv3::Mv3UnknownDataInMesh`] (see
/// [`Mv3ExtrasUnknownDataInFile`] for why a shadow type is needed).
#[derive(Debug, Default, Clone, Copy, Deserialize)]
struct Mv3ExtrasUnknownDataInMesh {
    u: u16,
    v: u16,
}

impl From<Mv3ExtrasUnknownDataInMesh> for Mv3UnknownDataInMesh {
    fn from(v: Mv3ExtrasUnknownDataInMesh) -> Self {
        Mv3UnknownDataInMesh { u: v.u, v: v.v }
    }
}

#[derive(Debug, Default, Deserialize)]
struct Mv3ExtrasMesh {
    #[serde(default)]
    unknown: u32,
    #[serde(default)]
    unknown_data: Vec<Mv3ExtrasUnknownDataInMesh>,
}

#[derive(Debug, Default, Deserialize)]
struct Mv3ExtrasModel {
    #[serde(default)]
    unknown: Vec<u8>,
    #[serde(default)]
    mesh_metadata: Vec<Mv3ExtrasMesh>,
}

#[derive(Debug, Default, Deserialize)]
struct Mv3Extras {
    #[serde(default = "default_version")]
    version: u32,
    #[serde(default)]
    action_desc: Vec<Mv3ExtrasAction>,
    #[serde(default)]
    textures: Vec<Mv3ExtrasTexture>,
    #[serde(default)]
    file_unknown_data: Vec<Mv3ExtrasUnknownDataInFile>,
    #[serde(default)]
    model_metadata: Vec<Mv3ExtrasModel>,
}

impl Mv3Extras {
    /// Builds a fallback "extras" tree directly from a replacement
    /// template's already-parsed [`Mv3File`] (see
    /// [`convert_with_template`]), used in place of a real
    /// `asset.extras.yaobow` payload when `scene` has none of its own.
    /// Reads straight off the real on-disk struct fields (no JSON round
    /// trip needed, unlike [`mv3_extras`]).
    fn from_file(file: &Mv3File) -> Self {
        Mv3Extras {
            version: file.version,
            action_desc: file
                .action_desc
                .iter()
                .map(|a| Mv3ExtrasAction {
                    tick: a.tick,
                    name: a.name.as_str().unwrap_or_default(),
                })
                .collect(),
            textures: file
                .textures
                .iter()
                .map(|t| Mv3ExtrasTexture {
                    unknown: t.unknown.clone(),
                    names: t
                        .names
                        .iter()
                        .map(|n| n.to_string().unwrap_or_default())
                        .collect(),
                })
                .collect(),
            file_unknown_data: file
                .unknown_data
                .iter()
                .map(|u| Mv3ExtrasUnknownDataInFile {
                    unknown0: u.unknown0.clone(),
                    unknown1: u.unknown1,
                    unknown2_count: u.unknown2_count,
                    unknown2: u.unknown2.clone(),
                })
                .collect(),
            model_metadata: file
                .models
                .iter()
                .map(|m| Mv3ExtrasModel {
                    unknown: m.unknown.clone(),
                    mesh_metadata: m
                        .meshes
                        .iter()
                        .map(|me| Mv3ExtrasMesh {
                            unknown: me.unknown,
                            unknown_data: me
                                .unknown_data
                                .iter()
                                .map(|d| Mv3ExtrasUnknownDataInMesh { u: d.u, v: d.v })
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

fn default_version() -> u32 {
    4
}

fn mv3_extras(scene: &ImportedScene, diagnostics: &mut Diagnostics) -> Option<Mv3Extras> {
    let extras = scene.extras.as_ref()?;
    if extras.target_format() != Some("mv3") {
        diagnostics.push(format!(
            "asset.extras.yaobow.payload.target_format is {:?}, not \"mv3\"; ignoring round-trip metadata",
            extras.target_format()
        ));
        return None;
    }
    match serde_json::from_value(extras.payload.clone()) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            diagnostics.push(format!(
                "failed to parse asset.extras.yaobow.payload as mv3 metadata ({err}); using defaults"
            ));
            None
        }
    }
}
