//! Normalized glTF scene → `.mv3` (animated role/prop model) converter.
//!
//! Mirrors [`crate::exporters::gltf::mv3`] in reverse. That exporter
//! emits one glTF node per `(model, mesh)` pair (so each part can carry
//! its own texture), tagging every such node with a
//! `node.extras.yaobow` `{model_index, mesh_index}` payload. This
//! converter regroups sibling scene-root nodes that share the same
//! `model_index` back into a single [`Mv3Model`] (see
//! [`group_mv3_nodes`]); every glTF `Primitive` across a group's nodes
//! becomes one [`Mv3Mesh`], with positions/UVs pooled 1:1 per glTF
//! vertex across the whole group (no cross-primitive dedup — simpler
//! and always correct, if a little larger than a hand-optimized
//! packer). Morph targets become additional per-vertex frame
//! snapshots; a `weights` animation channel on any node in the group
//! supplies the per-frame tick timestamps (every node in one model
//! shares the same timeline, matching the exporter's single shared time
//! accessor).
//!
//! When a scene's mesh-bearing root nodes don't *all* carry the
//! `model_index`/`mesh_index` tag (a plain, hand-authored glTF, or one
//! authored by an older exporter version), this converter falls back to
//! the original convention instead: one root mesh node = one model,
//! with each of that node's `Primitive`s becoming one [`Mv3Mesh`].
//!
//! # Supported scene shape (explicit constraints)
//!
//! MV3 has **no node hierarchy or node-level transform** — a model is
//! just a bag of meshes sharing one vertex-frame pool. This converter
//! therefore requires:
//! * every mesh-bearing node designated as a model must be a **scene
//!   root** ([`ImportError::NestedMeshNode`]) — nesting a mesh under a
//!   transform node has no MV3 representation;
//! * that node's static scale must be **uniform**
//!   ([`ImportError::NonUniformScale`]) — its static translation/
//!   rotation/scale is baked directly into the vertex/normal data;
//! * the node must **not** be targeted by a TRS animation channel
//!   ([`ImportError::UnsupportedAnimationTarget`]) — MV3 can only animate
//!   vertex positions (morph targets), never a node transform;
//! * every primitive of one node's mesh must agree on morph target count
//!   ([`ImportError::MorphTargetCountMismatch`]), and if there are any
//!   morph targets, a `weights` animation channel on that node must
//!   supply exactly `target_count + 1` keyframe times
//!   ([`ImportError::MissingWeightsAnimation`],
//!   [`ImportError::MorphTargetTimingMismatch`]);
//! * every primitive's vertex count must fit in a `u16` index
//!   ([`ImportError::TooManyVertices`]), and quantized positions must fit
//!   in `i16` after [`Mv3Options::vertex_scale`]
//!   ([`ImportError::QuantizationOverflow`]).
//!
//! Round-tripping a model exported by [`crate::exporters::gltf::mv3`]
//! recovers the original reserved/"unknown" fields and action table from
//! `asset.extras.yaobow` (see [`crate::importers::extras`]); plain,
//! hand-authored glTF gets sensible zeroed defaults instead.

use std::collections::HashSet;
use std::io::{Seek, Write};

use fileformats::mv3::{
    Mv3ActionDesc, Mv3File, Mv3Frame, Mv3Mesh, Mv3Model, Mv3Texture, Mv3Triangle,
    Mv3UnknownDataInFile, Mv3UnknownDataInMesh, write_mv3,
};
use serde::Deserialize;
use serde_json::Value;

use super::error::{Diagnostics, ImportError};
use super::scene::ImportedScene;
use super::target::{ImportOptions, Mv3Options};
use super::{assert_no_trs_animation, quantize_world, rotate_normal, uniform_scale};

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

    let roots: HashSet<usize> = super::effective_roots(scene).into_iter().collect();

    let mesh_node_indices: Vec<usize> = scene
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.mesh.is_some())
        .map(|(index, _)| index)
        .collect();
    for &index in &mesh_node_indices {
        if !roots.contains(&index) {
            return Err(ImportError::NestedMeshNode {
                node: scene.nodes[index].name.clone(),
                target: "mv3",
            });
        }
        assert_no_trs_animation(scene, index, "mv3")?;
    }

    let groups = group_mv3_nodes(scene, &mesh_node_indices);

    let mut textures = Vec::new();
    let mut models = Vec::new();
    for node_indices in &groups {
        let model_metadata = extras
            .as_ref()
            .and_then(|e| e.model_metadata.get(models.len()));
        let (model, texture_name) =
            build_model(scene, node_indices, opts, &mut diagnostics, model_metadata)?;

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
                    texture.names = t
                        .names
                        .iter()
                        .map(|n| super::gbk_sized_string(n))
                        .collect::<Result<Vec<_>, _>>()?;
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

/// Groups `mesh_node_indices` (every mesh-bearing scene-root node) into
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
/// simpler convention: one root mesh node = one model, with that node's
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

/// Builds one [`Mv3Model`] from `node_indices` (one or more scene-root
/// nodes belonging to the same original model — see [`group_mv3_nodes`]).
/// Every primitive of every node in the group becomes one [`Mv3Mesh`];
/// positions/UVs/morph deltas are pooled into one shared per-model
/// vertex frame pool (MV3 triangle indices reference that pool
/// directly, not a per-mesh-local one).
fn build_model(
    scene: &ImportedScene,
    node_indices: &[usize],
    opts: &Mv3Options,
    diagnostics: &mut Diagnostics,
    model_metadata: Option<&Mv3ExtrasModel>,
) -> Result<(Mv3Model, Option<String>), ImportError> {
    let model_name = scene.nodes[node_indices[0]].name.clone();

    let mut target_count: Option<usize> = None;
    for &node_index in node_indices {
        let node = &scene.nodes[node_index];
        let mesh = node.mesh.as_ref().expect("caller checked mesh.is_some()");
        for p in &mesh.primitives {
            let n = p.morph_targets.len();
            match target_count {
                None => target_count = Some(n),
                Some(tc) if tc != n => {
                    return Err(ImportError::MorphTargetCountMismatch {
                        node: node.name.clone(),
                        a: tc,
                        b: n,
                    });
                }
                _ => {}
            }
        }
    }
    let target_count = target_count.unwrap_or(0);
    let frame_count = target_count + 1;

    let frame_times: Vec<f32> = if target_count > 0 {
        let channel = scene
            .animations
            .iter()
            .flat_map(|a| a.weight_channels.iter())
            .find(|c| node_indices.contains(&c.node))
            .ok_or_else(|| ImportError::MissingWeightsAnimation {
                node: model_name.clone(),
                frames: frame_count,
            })?;
        if channel.times.len() != frame_count {
            return Err(ImportError::MorphTargetTimingMismatch {
                node: model_name.clone(),
                targets: target_count,
                expected: channel.times.len(),
                targets_plus_one: frame_count,
            });
        }
        channel.times.clone()
    } else {
        vec![0.0]
    };

    let mut pooled_positions: Vec<[f32; 3]> = Vec::new();
    let mut pooled_normals: Vec<Option<[f32; 3]>> = Vec::new();
    let mut pooled_uvs: Vec<[f32; 2]> = Vec::new();
    // `pooled_deltas[k]` holds the per-vertex position delta for morph
    // target `k` (frame `k + 1`).
    let mut pooled_deltas: Vec<Vec<[f32; 3]>> = vec![Vec::new(); target_count];
    // Parallel to `pooled_positions`: the (node index, uniform scale) a
    // pooled vertex's static transform should be baked from — each node
    // in the group may carry its own (typically identity) transform.
    let mut pooled_transform: Vec<(usize, f32)> = Vec::new();

    let mut meshes = Vec::new();
    let mut texture_name = None;
    for &node_index in node_indices {
        let node = &scene.nodes[node_index];
        let mesh = node.mesh.as_ref().expect("caller checked mesh.is_some()");
        let scale = uniform_scale(node, "mv3")?;

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

            let base_index = pooled_positions.len() as u32;
            for i in 0..vertex_count {
                pooled_positions.push(primitive.positions[i]);
                pooled_normals.push(primitive.normals.get(i).copied());
                pooled_uvs.push(primitive.uv0.get(i).copied().unwrap_or([0.0, 0.0]));
                pooled_transform.push((node_index, scale));
                for k in 0..target_count {
                    let delta = primitive.morph_targets[k]
                        .position_deltas
                        .get(i)
                        .copied()
                        .unwrap_or([0.0, 0.0, 0.0]);
                    pooled_deltas[k].push(delta);
                }
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
                            vertex_count: pooled_positions.len(),
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

    let vertex_per_frame = pooled_positions.len() as u32;
    if pooled_positions.is_empty() {
        diagnostics.push(format!(
            "model `{}` produced no vertices; emitting an empty model",
            model_name
        ));
    }

    let mut aabb_min = if pooled_positions.is_empty() {
        [0.0; 3]
    } else {
        [f32::INFINITY; 3]
    };
    let mut aabb_max = if pooled_positions.is_empty() {
        [0.0; 3]
    } else {
        [f32::NEG_INFINITY; 3]
    };
    let mut frames = Vec::with_capacity(frame_count);
    for k in 0..frame_count {
        let timestamp = (frame_times[k] * opts.ticks_per_second).round() as u32;
        let mut vertices = Vec::with_capacity(pooled_positions.len());
        for i in 0..pooled_positions.len() {
            let (node_index, scale) = pooled_transform[i];
            let node = &scene.nodes[node_index];
            let mut p = pooled_positions[i];
            if k > 0 {
                let d = pooled_deltas[k - 1][i];
                p = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
            }
            p = quantize_world(p, node, scale);
            for axis in 0..3 {
                aabb_min[axis] = aabb_min[axis].min(p[axis]);
                aabb_max[axis] = aabb_max[axis].max(p[axis]);
            }
            let (x, y, z) = quantize_i16(p, opts.vertex_scale, &node.name)?;
            let normal = pooled_normals[i]
                .map(|n| rotate_normal(n, node.rotation))
                .unwrap_or([0.0, 1.0, 0.0]);
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
            timestamp,
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
        frame_count: frame_count as u32,
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
