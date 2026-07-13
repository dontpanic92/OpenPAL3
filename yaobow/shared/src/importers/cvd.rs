//! Normalized glTF scene → `.cvd` (composite scene model: a node hierarchy
//! where each part may carry its own morph-target animation and TRS
//! keyframes) converter.
//!
//! Mirrors [`crate::exporters::gltf::cvd`] in reverse, built directly on
//! top of the byte-level [`fileformats::pal3::cvd`] reader/writer (which
//! replaced an originally-planned hand-rolled CVD writer once that module
//! appeared from concurrent work on this repository).
//!
//! Every glTF node becomes one [`CvdModelNode`]; a node with a mesh gets a
//! [`CvdModel`] (one [`fileformats::pal3::cvd::CvdMaterial`] per glTF
//! primitive, vertices pooled 1:1 per glTF vertex and shared across the
//! model's per-frame vertex-animation snapshots, matching
//! [`crate::importers::mv3`]/[`crate::importers::pol`]'s pooling
//! strategy); nodes with no mesh become pure hierarchy "join" nodes
//! (`model: None`). Since the exporter puts every material's `Primitive`
//! for one node on the *same* shared position/normal/UV/morph-target
//! accessors, primitives whose vertex data is bit-identical to an
//! already-processed primitive of the same node (see
//! [`super::scene::ImportedPrimitive::shares_vertex_data`]) reuse that
//! block instead of appending a duplicate copy of the whole vertex
//! buffer; a primitive with genuinely distinct vertex data (plain,
//! hand-authored glTF) still gets its own appended block.
//!
//! # Coordinate/keyframe encoding
//!
//! CVD stores several fields "raw": which of ten per-keyframe `unknown`
//! floats hold the actual position/rotation, and in what axis convention,
//! depends on a per-track `version` byte whose interpretation lives in
//! `shared::openpal3::loaders::cvd_loader` (kept engine-agnostic in
//! `fileformats`). This converter targets keyframe **version 3** for
//! position/rotation and **version 1** for scale (the simplest field
//! layout each version family supports) using the *inverse* of that
//! module's documented decode formulas:
//! * vertex position: raw `(px, py, pz)` decodes to `(px, pz, -py)`, so
//!   encoding `(X, Y, Z)` writes `(X, -Z, Y)`. Normals and UVs pass
//!   through unchanged (no axis swap at all).
//! * position keyframe (v3): decode is `(u1, u3, -u2)`, so encoding
//!   `(X, Y, Z)` writes `u1=X, u2=-Z, u3=Y`.
//! * rotation keyframe (v2/v3): decode is
//!   `(-u1, -u3, u2, u4)` (swap yz, negate z, conjugate), so encoding
//!   `(x, y, z, w)` writes `u1=-x, u2=z, u3=-y, u4=w`.
//! * scale keyframe (v1): decode is quaternion `(u9, u11, -u10, u12)` and
//!   scale `(u6, u8, u7)`. glTF's `scale` channel carries no rotation
//!   component, so the quaternion half is always written as identity
//!   (`u9=u10=u11=0, u12=1`); encoding scale `(sx, sy, sz)` writes
//!   `u6=sx, u7=sz, u8=sy`.
//!
//! # Supported scene shape (explicit constraints)
//!
//! * a node's static scale must be **uniform**
//!   ([`ImportError::NonUniformScale`]) — CVD only stores one
//!   `scale_factor` per model, independent of any animated scale
//!   keyframes;
//! * a node with **no mesh** has no field to store *any* transform at
//!   all (not even a static one) — a non-identity static transform or an
//!   animation channel targeting a mesh-less node is rejected
//!   ([`ImportError::UnsupportedStaticTransform`] /
//!   [`ImportError::UnsupportedAnimationTarget`]);
//! * a mesh-bearing node's static translation/rotation is only
//!   representable via a **single-keyframe** position/rotation track when
//!   there's no corresponding animation channel; an actual animation
//!   channel must use **LINEAR** interpolation
//!   ([`ImportError::UnsupportedInterpolation`]);
//! * every primitive of one node's mesh must agree on morph target count
//!   ([`ImportError::MorphTargetCountMismatch`]), and if there are any, a
//!   `weights` animation channel on that node must supply exactly
//!   `target_count + 1` keyframe times
//!   ([`ImportError::MissingWeightsAnimation`],
//!   [`ImportError::MorphTargetTimingMismatch`]);
//! * every primitive's vertex count must fit in a `u16` index
//!   ([`ImportError::TooManyVertices`]) — CVD, like MV3/POL, uses `u16`
//!   triangle indices.
//!
//! Round-tripping a model exported by [`crate::exporters::gltf::cvd`]
//! recovers each keyframe's exact raw `unknown` floats and each
//! material's `color1..4`/`unknown_byte` from `asset.extras.yaobow` by
//! walking it in lockstep with the glTF node tree (matched positionally:
//! root order, then each node's `children` order — the same order the
//! exporter used to build it). A few raw fields the *exporter's own*
//! engine-facing types never captured in the first place — each model's
//! trailing 4x4 `matrix`, `CvdMaterial::unknown_float2`, and the full
//! per-material V2 `extra` block's contents — can't be round-tripped
//! through extras at all and always get a default (`matrix` = identity,
//! `unknown_float2` = 0.0, `extra` = empty) regardless of the source
//! file; this is a pre-existing limitation of
//! [`crate::exporters::gltf::cvd`], not something this converter can
//! recover.

use std::io::Write;

use fileformats::pal3::cvd::{
    CvdFile, CvdMaterial, CvdMaterialExtra, CvdMesh, CvdModel, CvdModelNode, CvdPositionKeyFrame,
    CvdPositionKeyFrames, CvdRotationKeyFrame, CvdRotationKeyFrames, CvdScaleKeyFrame,
    CvdScaleKeyFrames, CvdTriangle, CvdVersion, CvdVertex, write_cvd,
};
use fileformats::rwbs::{Matrix44f, TexCoord};
use serde::Deserialize;

use super::error::{Diagnostics, ImportError};
use super::scene::{ImportedNode, ImportedScene, ImportedTrsChannel, Interpolation, TrsProperty};
use super::target::ImportOptions;

/// Converts `scene` into an in-memory [`CvdFile`], applying `options.cvd`.
pub fn convert(
    scene: &ImportedScene,
    options: &ImportOptions,
) -> Result<(CvdFile, Diagnostics), ImportError> {
    convert_with_template(scene, options, None)
}

/// Like [`convert`], but falls back to `template`'s opaque metadata
/// (magic/keyframe raw arrays/material colors & texture names/per-frame
/// timing) wherever `scene` has no `asset.extras.yaobow` round-trip
/// metadata of its own, walked in lockstep with the glTF node tree the
/// same way [`cvd_extras`]-sourced metadata is. `extras`, when present,
/// always takes precedence over `template`.
pub fn convert_with_template(
    scene: &ImportedScene,
    options: &ImportOptions,
    template: Option<&CvdFile>,
) -> Result<(CvdFile, Diagnostics), ImportError> {
    let mut diagnostics = Diagnostics::default();
    let mut extras = cvd_extras(scene, &mut diagnostics);
    if extras.is_none() {
        if let Some(template) = template {
            diagnostics.push(
                "no asset.extras.yaobow round-trip metadata found; falling back to the \
                 replacement template's opaque metadata"
                    .to_string(),
            );
            extras = Some(CvdExtras::from_file(template));
        }
    }

    let version = match extras.as_ref().and_then(|e| e.magic) {
        Some(magic) if &magic == b"cvds" => CvdVersion::V2,
        Some(magic) if &magic == b"cvdf" => CvdVersion::V1,
        _ => {
            if options.cvd.legacy_magic {
                CvdVersion::V1
            } else {
                CvdVersion::V2
            }
        }
    };

    let roots = super::effective_roots(scene);
    let extras_roots: &[CvdExtrasModelNode] =
        extras.as_ref().map(|e| e.models.as_slice()).unwrap_or(&[]);

    let mut models = Vec::with_capacity(roots.len());
    for (i, &node_index) in roots.iter().enumerate() {
        models.push(build_node(
            scene,
            node_index,
            version,
            &mut diagnostics,
            extras_roots.get(i),
        )?);
    }

    if models.is_empty() {
        return Err(ImportError::NoGeometry("cvd"));
    }

    Ok((CvdFile { version, models }, diagnostics))
}

/// Converts `scene` and writes the resulting `.cvd` bytes to `writer`.
pub fn write(
    scene: &ImportedScene,
    options: &ImportOptions,
    writer: &mut impl Write,
) -> Result<Diagnostics, ImportError> {
    let (file, diagnostics) = convert(scene, options)?;
    write_cvd(writer, &file)?;
    Ok(diagnostics)
}

fn build_node(
    scene: &ImportedScene,
    node_index: usize,
    version: CvdVersion,
    diagnostics: &mut Diagnostics,
    extras: Option<&CvdExtrasModelNode>,
) -> Result<CvdModelNode, ImportError> {
    let node = &scene.nodes[node_index];

    let model = if node.mesh.is_some() {
        Some(build_model(
            scene,
            node_index,
            node,
            version,
            diagnostics,
            extras.and_then(|e| e.model.as_ref()),
        )?)
    } else {
        // A mesh-less "join" node has no field anywhere to store a
        // transform, static or animated.
        if !node.is_identity_transform() {
            return Err(ImportError::UnsupportedStaticTransform {
                node: node.name.clone(),
                target: "cvd",
            });
        }
        super::assert_no_trs_animation(scene, node_index, "cvd")?;
        None
    };

    let extras_children: &[CvdExtrasModelNode] =
        extras.and_then(|e| e.children.as_deref()).unwrap_or(&[]);
    let mut children = Vec::with_capacity(node.children.len());
    for (i, &child_index) in node.children.iter().enumerate() {
        children.push(build_node(
            scene,
            child_index,
            version,
            diagnostics,
            extras_children.get(i),
        )?);
    }

    Ok(CvdModelNode { model, children })
}

fn build_model(
    scene: &ImportedScene,
    node_index: usize,
    node: &ImportedNode,
    version: CvdVersion,
    diagnostics: &mut Diagnostics,
    extras: Option<&CvdExtrasModel>,
) -> Result<CvdModel, ImportError> {
    let scale_factor = super::uniform_scale(node, "cvd")?;

    let translation_channel = find_trs_channel(scene, node_index, TrsProperty::Translation, "cvd")?;
    let rotation_channel = find_trs_channel(scene, node_index, TrsProperty::Rotation, "cvd")?;
    let scale_channel = find_trs_channel(scene, node_index, TrsProperty::Scale, "cvd")?;

    let position_keyframes = build_position_keyframes(
        node,
        translation_channel,
        extras.and_then(|e| e.position_keyframes.as_ref()),
        diagnostics,
    );
    let rotation_keyframes = build_rotation_keyframes(
        node,
        rotation_channel,
        extras.and_then(|e| e.rotation_keyframes.as_ref()),
        diagnostics,
    );
    let scale_keyframes = build_scale_keyframes(
        scale_channel,
        extras.and_then(|e| e.scale_keyframes.as_ref()),
        diagnostics,
    );

    let mesh = build_mesh(
        scene,
        node_index,
        node,
        version,
        diagnostics,
        extras.map(|e| &e.mesh),
    )?;

    Ok(CvdModel {
        position_keyframes,
        rotation_keyframes,
        scale_keyframes,
        scale_factor,
        mesh,
        // Never round-trippable: `exporters::gltf::cvd` reads from
        // `cvd_loader::CvdModel`, which drops this field entirely.
        matrix: Matrix44f([
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ]),
    })
}

fn build_mesh(
    scene: &ImportedScene,
    node_index: usize,
    node: &ImportedNode,
    version: CvdVersion,
    diagnostics: &mut Diagnostics,
    extras: Option<&CvdExtrasMesh>,
) -> Result<CvdMesh, ImportError> {
    let mesh = node.mesh.as_ref().expect("caller checked mesh.is_some()");

    let target_count = mesh
        .primitives
        .first()
        .map(|p| p.morph_targets.len())
        .unwrap_or(0);
    for p in &mesh.primitives {
        if p.morph_targets.len() != target_count {
            return Err(ImportError::MorphTargetCountMismatch {
                node: node.name.clone(),
                a: target_count,
                b: p.morph_targets.len(),
            });
        }
    }
    let frame_count = target_count + 1;

    let frame_times: Vec<f32> = if target_count > 0 {
        let channel = scene
            .animations
            .iter()
            .flat_map(|a| a.weight_channels.iter())
            .find(|c| c.node == node_index)
            .ok_or_else(|| ImportError::MissingWeightsAnimation {
                node: node.name.clone(),
                frames: frame_count,
            })?;
        if channel.times.len() != frame_count {
            return Err(ImportError::MorphTargetTimingMismatch {
                node: node.name.clone(),
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
    let mut pooled_normals: Vec<[f32; 3]> = Vec::new();
    let mut pooled_uvs: Vec<[f32; 2]> = Vec::new();
    let mut pooled_position_deltas: Vec<Vec<[f32; 3]>> = vec![Vec::new(); target_count];
    let mut pooled_normal_deltas: Vec<Vec<[f32; 3]>> = vec![Vec::new(); target_count];
    // The base index each already-processed primitive's vertex data
    // starts at in the pool above, parallel to `mesh.primitives` (see
    // `ImportedPrimitive::shares_vertex_data`'s doc comment for why a
    // later primitive may reuse an earlier one's block instead of
    // appending a new one).
    let mut base_index_for_primitive: Vec<u32> = Vec::with_capacity(mesh.primitives.len());

    // The exporter serializes *every* `CvdMaterial` (including ones with
    // no triangles) into `extras.mesh.materials`, but skips those same
    // empty-triangle materials when building glTF primitives. Filter out
    // the empty ones here so this list lines up positionally with
    // `mesh.primitives` (which only ever contains what the exporter
    // actually emitted) before we index it by `prim_index` below.
    let filtered_materials: Vec<&CvdExtrasMaterial> = extras
        .map(|e| {
            e.materials
                .iter()
                .filter(|m| m.triangle_count > 0)
                .collect()
        })
        .unwrap_or_default();

    let mut materials = Vec::with_capacity(mesh.primitives.len());
    for (prim_index, primitive) in mesh.primitives.iter().enumerate() {
        let vertex_count = primitive.positions.len();
        if vertex_count > u16::MAX as usize + 1 {
            return Err(ImportError::TooManyVertices {
                mesh: node.name.clone(),
                primitive: prim_index,
                count: vertex_count,
            });
        }

        let base_index = mesh.primitives[..prim_index]
            .iter()
            .position(|prev| prev.shares_vertex_data(primitive))
            .map(|prev_index| base_index_for_primitive[prev_index]);
        let base_index = match base_index {
            Some(base_index) => base_index,
            None => {
                let base_index = pooled_positions.len() as u32;
                for i in 0..vertex_count {
                    let normal = primitive.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
                    pooled_positions.push(primitive.positions[i]);
                    pooled_normals.push(normal);
                    pooled_uvs.push(primitive.uv0.get(i).copied().unwrap_or([0.0, 0.0]));
                    for k in 0..target_count {
                        let pd = primitive.morph_targets[k]
                            .position_deltas
                            .get(i)
                            .copied()
                            .unwrap_or([0.0, 0.0, 0.0]);
                        pooled_position_deltas[k].push(pd);
                        let nd = primitive.morph_targets[k]
                            .normal_deltas
                            .as_ref()
                            .and_then(|d| d.get(i).copied())
                            .unwrap_or([0.0, 0.0, 0.0]);
                        pooled_normal_deltas[k].push(nd);
                    }
                }
                base_index
            }
        };
        base_index_for_primitive.push(base_index);

        if primitive.indices.len() % 3 != 0 {
            return Err(ImportError::Other(format!(
                "node `{}` primitive #{prim_index} has {} indices, not a multiple of 3",
                node.name,
                primitive.indices.len()
            )));
        }
        let mut triangles = Vec::with_capacity(primitive.indices.len() / 3);
        for tri in primitive.indices.chunks_exact(3) {
            let mut idx = [0u16; 3];
            for (slot, &raw) in idx.iter_mut().zip(tri) {
                let local =
                    raw.checked_add(base_index)
                        .ok_or_else(|| ImportError::IndexRemapOverflow {
                            mesh: node.name.clone(),
                            primitive: prim_index,
                            index: raw,
                            base_index,
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
            triangles.push(CvdTriangle { indices: idx });
        }

        let mat_extra = filtered_materials.get(prim_index).copied();
        // A replacement template's texture name (when present) takes
        // precedence over the glTF material's texture, mirroring
        // `importers::pol`'s extras-texture-name precedence — see
        // `CvdExtrasMaterial::texture_name`'s doc comment for why this
        // only ever comes from a template, never real extras JSON.
        let texture_name = mat_extra
            .and_then(|m| m.texture_name.clone())
            .or_else(|| primitive.material_texture.clone())
            .unwrap_or_default();
        materials.push(CvdMaterial {
            unknown_byte: mat_extra.map(|m| m.unknown_byte).unwrap_or(0),
            color1: mat_extra.map(|m| m.color1).unwrap_or(0xFFFFFFFF),
            color2: mat_extra.map(|m| m.color2).unwrap_or(0xFFFFFFFF),
            color3: mat_extra.map(|m| m.color3).unwrap_or(0xFFFFFFFF),
            color4: mat_extra.map(|m| m.color4).unwrap_or(0xFFFFFFFF),
            unknown_float2: 0.0,
            texture_name,
            triangle_count: triangles.len() as u32,
            triangles,
            extra: if version.has_material_extra() {
                Some(CvdMaterialExtra {
                    values: Vec::new(),
                    blocks: Vec::new(),
                })
            } else {
                None
            },
        });
        if mat_extra.is_none() {
            diagnostics.push(format!(
                "node `{}` primitive #{prim_index} has no round-trip color metadata; defaulting to opaque white",
                node.name
            ));
        }
    }

    if pooled_positions.is_empty() {
        diagnostics.push(format!(
            "node `{}` produced no vertices; emitting an empty mesh",
            node.name
        ));
    }

    let vertex_count = pooled_positions.len() as u32;
    let mut frames = Vec::with_capacity(frame_count);
    for k in 0..frame_count {
        let mut vertices = Vec::with_capacity(pooled_positions.len());
        for i in 0..pooled_positions.len() {
            let mut p = pooled_positions[i];
            let mut n = pooled_normals[i];
            if k > 0 {
                let pd = pooled_position_deltas[k - 1][i];
                p = [p[0] + pd[0], p[1] + pd[1], p[2] + pd[2]];
                let nd = pooled_normal_deltas[k - 1][i];
                n = [n[0] + nd[0], n[1] + nd[1], n[2] + nd[2]];
            }
            vertices.push(CvdVertex {
                tex_coord: TexCoord {
                    u: pooled_uvs[i][0],
                    v: pooled_uvs[i][1],
                },
                normal: n,
                position: encode_vertex_position(p),
            });
        }
        frames.push(vertices);
    }

    let frame_extra = extras
        .map(|e| e.unknown_data.clone())
        .filter(|d| d.len() == frame_count)
        .unwrap_or_else(|| frame_times.clone());

    Ok(CvdMesh {
        frame_count: frame_count as u32,
        vertex_count,
        frames,
        frame_extra,
        material_count: materials.len() as u32,
        materials,
    })
}

/// Raw vertex position encode: inverse of `cvd_loader::convert_vertex`'s
/// `Vec3::new(px, pz, -py)` decode.
fn encode_vertex_position(p: [f32; 3]) -> [f32; 3] {
    [p[0], -p[2], p[1]]
}

fn find_trs_channel<'a>(
    scene: &'a ImportedScene,
    node_index: usize,
    property: TrsProperty,
    target: &'static str,
) -> Result<Option<&'a ImportedTrsChannel>, ImportError> {
    let mut found: Option<(&str, &ImportedTrsChannel)> = None;
    for anim in &scene.animations {
        for ch in &anim.trs_channels {
            if ch.node == node_index && ch.property == property {
                if ch.interpolation != Interpolation::Linear {
                    return Err(ImportError::UnsupportedInterpolation {
                        animation: anim.name.clone(),
                        node: scene.nodes[node_index].name.clone(),
                        interpolation: match ch.interpolation {
                            Interpolation::Linear => gltf::animation::Interpolation::Linear,
                            Interpolation::Step => gltf::animation::Interpolation::Step,
                        },
                        target,
                    });
                }
                found = Some((anim.name.as_str(), ch));
            }
        }
    }
    Ok(found.map(|(_, ch)| ch))
}

/// Inverse of `cvd_loader::convert_position_keyframes`'s version-3 decode
/// (`(u1, u3, -u2)`).
fn encode_position_keyframe(timestamp: f32, p: [f32; 3]) -> CvdPositionKeyFrame {
    let mut unknown = [0f32; 10];
    unknown[1] = p[0];
    unknown[2] = -p[2];
    unknown[3] = p[1];
    CvdPositionKeyFrame { timestamp, unknown }
}

/// Inverse of `cvd_loader::convert_rotation_keyframes`'s version-2/3 decode
/// (`(-u1, -u3, u2, u4)`, i.e. swap yz, negate z, then conjugate).
fn encode_rotation_keyframe(timestamp: f32, q: [f32; 4]) -> CvdRotationKeyFrame {
    let mut unknown = [0f32; 10];
    unknown[1] = -q[0];
    unknown[2] = q[2];
    unknown[3] = -q[1];
    unknown[4] = q[3];
    CvdRotationKeyFrame { timestamp, unknown }
}

/// Inverse of `cvd_loader::convert_scale_keyframes`'s version-1 decode
/// (quaternion `(u9, u11, -u10, u12)`, scale `(u6, u8, u7)`). The
/// quaternion half is always written as identity: glTF's `scale` channel
/// has no rotation component.
fn encode_scale_keyframe(timestamp: f32, s: [f32; 3]) -> CvdScaleKeyFrame {
    let mut unknown = [0f32; 14];
    unknown[6] = s[0];
    unknown[7] = s[2];
    unknown[8] = s[1];
    unknown[9] = 0.0;
    unknown[10] = 0.0;
    unknown[11] = 0.0;
    unknown[12] = 1.0;
    CvdScaleKeyFrame { timestamp, unknown }
}

fn build_position_keyframes(
    node: &ImportedNode,
    channel: Option<&ImportedTrsChannel>,
    extra: Option<&CvdExtrasKeyFrames>,
    diagnostics: &mut Diagnostics,
) -> Option<CvdPositionKeyFrames> {
    match channel {
        Some(ch) => {
            let use_extra = extra
                .map(|e| e.frames.len() == ch.times.len())
                .unwrap_or(false);
            if extra.is_some() && !use_extra {
                diagnostics.push(format!(
                    "node `{}` position keyframe count in extras doesn't match its glTF animation channel; re-deriving raw values instead of restoring them",
                    node.name
                ));
            }
            let version = if use_extra { extra.unwrap().version } else { 3 };
            let frames = ch
                .times
                .iter()
                .zip(ch.values.iter())
                .enumerate()
                .map(|(i, (&t, v))| {
                    if use_extra {
                        let raw = &extra.unwrap().frames[i];
                        CvdPositionKeyFrame {
                            timestamp: t,
                            unknown: raw.as_array(),
                        }
                    } else {
                        encode_position_keyframe(t, [v[0], v[1], v[2]])
                    }
                })
                .collect();
            Some(CvdPositionKeyFrames { version, frames })
        }
        None if node.translation != [0.0; 3] => Some(CvdPositionKeyFrames {
            version: 3,
            frames: vec![encode_position_keyframe(0.0, node.translation)],
        }),
        None => None,
    }
}

fn build_rotation_keyframes(
    node: &ImportedNode,
    channel: Option<&ImportedTrsChannel>,
    extra: Option<&CvdExtrasKeyFrames>,
    diagnostics: &mut Diagnostics,
) -> Option<CvdRotationKeyFrames> {
    match channel {
        Some(ch) => {
            let use_extra = extra
                .map(|e| e.frames.len() == ch.times.len())
                .unwrap_or(false);
            if extra.is_some() && !use_extra {
                diagnostics.push(format!(
                    "node `{}` rotation keyframe count in extras doesn't match its glTF animation channel; re-deriving raw values instead of restoring them",
                    node.name
                ));
            }
            let version = if use_extra { extra.unwrap().version } else { 3 };
            let frames = ch
                .times
                .iter()
                .zip(ch.values.iter())
                .enumerate()
                .map(|(i, (&t, v))| {
                    if use_extra {
                        let raw = &extra.unwrap().frames[i];
                        CvdRotationKeyFrame {
                            timestamp: t,
                            unknown: raw.as_array(),
                        }
                    } else {
                        encode_rotation_keyframe(t, *v)
                    }
                })
                .collect();
            Some(CvdRotationKeyFrames { version, frames })
        }
        None if node.rotation != [0.0, 0.0, 0.0, 1.0] => Some(CvdRotationKeyFrames {
            version: 3,
            frames: vec![encode_rotation_keyframe(0.0, node.rotation)],
        }),
        None => None,
    }
}

fn build_scale_keyframes(
    channel: Option<&ImportedTrsChannel>,
    extra: Option<&CvdExtrasScaleKeyFrames>,
    diagnostics: &mut Diagnostics,
) -> Option<CvdScaleKeyFrames> {
    let ch = channel?;
    let use_extra = extra
        .map(|e| e.frames.len() == ch.times.len())
        .unwrap_or(false);
    if extra.is_some() && !use_extra {
        diagnostics.push(
            "a scale keyframe count in extras doesn't match its glTF animation channel; re-deriving raw values instead of restoring them".to_string(),
        );
    }
    let version = if use_extra { extra.unwrap().version } else { 1 };
    let frames = ch
        .times
        .iter()
        .zip(ch.values.iter())
        .enumerate()
        .map(|(i, (&t, v))| {
            if use_extra {
                let raw = &extra.unwrap().frames[i];
                CvdScaleKeyFrame {
                    timestamp: t,
                    unknown: raw.as_array(),
                }
            } else {
                encode_scale_keyframe(t, [v[0], v[1], v[2]])
            }
        })
        .collect();
    Some(CvdScaleKeyFrames { version, frames })
}

/// Mirrors the flat `unknown1..unknown10` fields
/// `openpal3::loaders::cvd_loader::CvdPositionKeyFrame`/`CvdRotationKeyFrame`
/// serialize to (rather than the raw `[f32; 10]` array, which isn't what
/// the exporter emits), used for both position and rotation extras since
/// both share the same on-disk shape.
#[derive(Debug, Default, Clone, Deserialize)]
struct CvdExtrasKeyFrame {
    #[serde(default)]
    unknown1: f32,
    #[serde(default)]
    unknown2: f32,
    #[serde(default)]
    unknown3: f32,
    #[serde(default)]
    unknown4: f32,
    #[serde(default)]
    unknown5: f32,
    #[serde(default)]
    unknown6: f32,
    #[serde(default)]
    unknown7: f32,
    #[serde(default)]
    unknown8: f32,
    #[serde(default)]
    unknown9: f32,
    #[serde(default)]
    unknown10: f32,
}

impl CvdExtrasKeyFrame {
    fn as_array(&self) -> [f32; 10] {
        [
            self.unknown1,
            self.unknown2,
            self.unknown3,
            self.unknown4,
            self.unknown5,
            self.unknown6,
            self.unknown7,
            self.unknown8,
            self.unknown9,
            self.unknown10,
        ]
    }

    fn from_array(a: [f32; 10]) -> Self {
        CvdExtrasKeyFrame {
            unknown1: a[0],
            unknown2: a[1],
            unknown3: a[2],
            unknown4: a[3],
            unknown5: a[4],
            unknown6: a[5],
            unknown7: a[6],
            unknown8: a[7],
            unknown9: a[8],
            unknown10: a[9],
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct CvdExtrasKeyFrames {
    #[serde(default)]
    version: u8,
    #[serde(default)]
    frames: Vec<CvdExtrasKeyFrame>,
}

/// Mirrors `cvd_loader::CvdScaleKeyFrame`'s flat `unknown: [f32; 14]`
/// field (already the shape we need, no relabeling required).
#[derive(Debug, Default, Clone, Deserialize)]
struct CvdExtrasScaleKeyFrame {
    #[serde(default)]
    unknown: Vec<f32>,
}

impl CvdExtrasScaleKeyFrame {
    fn as_array(&self) -> [f32; 14] {
        let mut out = [0f32; 14];
        for (dst, src) in out.iter_mut().zip(self.unknown.iter()) {
            *dst = *src;
        }
        out
    }

    fn from_array(a: [f32; 14]) -> Self {
        CvdExtrasScaleKeyFrame {
            unknown: a.to_vec(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct CvdExtrasScaleKeyFrames {
    #[serde(default)]
    version: u8,
    #[serde(default)]
    frames: Vec<CvdExtrasScaleKeyFrame>,
}

#[derive(Debug, Default, Deserialize)]
struct CvdExtrasMaterial {
    #[serde(default)]
    unknown_byte: u8,
    #[serde(default)]
    color1: u32,
    #[serde(default)]
    color2: u32,
    #[serde(default)]
    color3: u32,
    #[serde(default)]
    color4: u32,
    /// Not part of the exporter's JSON shape (defaults to `None` and is
    /// simply absent from any real `asset.extras.yaobow` payload) — only
    /// populated by [`CvdExtras::from_file`]'s replacement-template
    /// fallback, where the template's raw [`CvdMaterial::texture_name`]
    /// is available directly (unlike extras, which never carries a
    /// texture name since the glTF material's own texture already
    /// supplies one).
    #[serde(default)]
    texture_name: Option<String>,
    /// Present in every real `asset.extras.yaobow` payload (the exporter
    /// serializes the whole `CvdMaterial`, unfiltered, into `models`).
    /// Materials with `triangle_count == 0` are skipped by the exporter
    /// when building glTF primitives, so this list is longer than — and
    /// misaligned with — the emitted `mesh.primitives`. We filter this
    /// list by `triangle_count > 0` before indexing it positionally by
    /// primitive index; see the filtering in `build_mesh`.
    #[serde(default)]
    triangle_count: u32,
}

#[derive(Debug, Default, Deserialize)]
struct CvdExtrasMesh {
    #[serde(default)]
    unknown_data: Vec<f32>,
    #[serde(default)]
    materials: Vec<CvdExtrasMaterial>,
}

#[derive(Debug, Default, Deserialize)]
struct CvdExtrasModel {
    #[serde(default)]
    position_keyframes: Option<CvdExtrasKeyFrames>,
    #[serde(default)]
    rotation_keyframes: Option<CvdExtrasKeyFrames>,
    #[serde(default)]
    scale_keyframes: Option<CvdExtrasScaleKeyFrames>,
    #[serde(default)]
    mesh: CvdExtrasMesh,
}

#[derive(Debug, Default, Deserialize)]
struct CvdExtrasModelNode {
    #[serde(default)]
    model: Option<CvdExtrasModel>,
    #[serde(default)]
    children: Option<Vec<CvdExtrasModelNode>>,
}

#[derive(Debug, Default, Deserialize)]
struct CvdExtras {
    #[serde(default)]
    magic: Option<[u8; 4]>,
    #[serde(default)]
    models: Vec<CvdExtrasModelNode>,
}

impl CvdExtras {
    /// Builds a fallback "extras" tree directly from a replacement
    /// template's already-parsed [`CvdFile`] (see
    /// [`convert_with_template`]), used in place of a real
    /// `asset.extras.yaobow` payload when `scene` has none of its own.
    /// Reads straight off the real on-disk struct fields (no JSON round
    /// trip needed, unlike [`cvd_extras`]), walking the template's node
    /// tree in the same root-order/children-order lockstep
    /// [`build_node`] expects.
    fn from_file(file: &CvdFile) -> Self {
        CvdExtras {
            magic: Some(match file.version {
                CvdVersion::V1 => *b"cvdf",
                CvdVersion::V2 => *b"cvds",
            }),
            models: file.models.iter().map(model_node_to_extras).collect(),
        }
    }
}

fn model_node_to_extras(node: &CvdModelNode) -> CvdExtrasModelNode {
    CvdExtrasModelNode {
        model: node.model.as_ref().map(model_to_extras),
        children: if node.children.is_empty() {
            None
        } else {
            Some(node.children.iter().map(model_node_to_extras).collect())
        },
    }
}

fn model_to_extras(model: &CvdModel) -> CvdExtrasModel {
    CvdExtrasModel {
        position_keyframes: model
            .position_keyframes
            .as_ref()
            .map(|k| CvdExtrasKeyFrames {
                version: k.version,
                frames: k
                    .frames
                    .iter()
                    .map(|f| CvdExtrasKeyFrame::from_array(f.unknown))
                    .collect(),
            }),
        rotation_keyframes: model
            .rotation_keyframes
            .as_ref()
            .map(|k| CvdExtrasKeyFrames {
                version: k.version,
                frames: k
                    .frames
                    .iter()
                    .map(|f| CvdExtrasKeyFrame::from_array(f.unknown))
                    .collect(),
            }),
        scale_keyframes: model
            .scale_keyframes
            .as_ref()
            .map(|k| CvdExtrasScaleKeyFrames {
                version: k.version,
                frames: k
                    .frames
                    .iter()
                    .map(|f| CvdExtrasScaleKeyFrame::from_array(f.unknown))
                    .collect(),
            }),
        mesh: mesh_to_extras(&model.mesh),
    }
}

fn mesh_to_extras(mesh: &CvdMesh) -> CvdExtrasMesh {
    CvdExtrasMesh {
        unknown_data: mesh.frame_extra.clone(),
        materials: mesh
            .materials
            .iter()
            .map(|m| CvdExtrasMaterial {
                unknown_byte: m.unknown_byte,
                color1: m.color1,
                color2: m.color2,
                color3: m.color3,
                color4: m.color4,
                texture_name: Some(m.texture_name.clone()),
                triangle_count: m.triangle_count,
            })
            .collect(),
    }
}

fn cvd_extras(scene: &ImportedScene, diagnostics: &mut Diagnostics) -> Option<CvdExtras> {
    let extras = scene.extras.as_ref()?;
    if extras.target_format() != Some("cvd") {
        diagnostics.push(format!(
            "asset.extras.yaobow.payload.target_format is {:?}, not \"cvd\"; ignoring round-trip metadata",
            extras.target_format()
        ));
        return None;
    }
    match serde_json::from_value(extras.payload.clone()) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            diagnostics.push(format!(
                "failed to parse asset.extras.yaobow.payload as cvd metadata ({err}); using defaults"
            ));
            None
        }
    }
}
