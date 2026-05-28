//! `cvd` (composite scene model, hierarchy of parts) → glTF.
//!
//! Each `CvdModelNode` becomes a glTF `Node`. Nodes that carry geometry
//! get a `Mesh`; geometry with multiple frames also exports a morph
//! target per frame (frame 0 is the base). Nodes with TRS keyframes
//! are wired up with LINEAR-interpolated `translation` / `rotation` /
//! `scale` animation channels, all combined into a single top-level
//! `Animation`.
//!
//! Coordinate-system and y/z swizzles applied by the cvd loader are
//! kept — we re-emit the data exactly as the engine sees it so the
//! viewer matches the in-game appearance.

use std::collections::BTreeMap;
use std::path::Path;

use gltf_json::accessor::Type as AccType;
use gltf_json::animation::{Channel, Interpolation, Property, Sampler, Target};
use gltf_json::mesh::{MorphTarget, Primitive, Semantic};
use gltf_json::validation::Checked;
use gltf_json::{Mesh, Node, Scene};
use mini_fs::MiniFs;

use crate::openpal3::loaders::cvd_loader::{
    CvdFile, CvdMesh, CvdModelNode, CvdPositionKeyFrames, CvdRotationKeyFrames, CvdScaleKeyFrames,
};

use super::glb::GlbBuilder;
use super::mv3::{build_material, flatten_vec2, flatten_vec3};

pub fn export_cvd_to_glb(
    cvd: &CvdFile,
    vfs: &MiniFs,
    model_path: &Path,
) -> anyhow::Result<Vec<u8>> {
    let mut b = GlbBuilder::new();
    let model_dir = model_path.parent().unwrap_or_else(|| Path::new(""));

    let mut animation_channels: Vec<Channel> = Vec::new();
    let mut animation_samplers: Vec<Sampler> = Vec::new();

    let mut root_nodes = Vec::new();
    for node in &cvd.models {
        let idx = build_node(
            &mut b,
            vfs,
            model_dir,
            node,
            &mut animation_channels,
            &mut animation_samplers,
        );
        root_nodes.push(idx);
    }

    let scene_idx = b.root.push(Scene {
        nodes: root_nodes,
        extensions: Default::default(),
        extras: Default::default(),
    });
    b.root.scene = Some(scene_idx);

    if !animation_channels.is_empty() {
        b.root.push(gltf_json::Animation {
            channels: animation_channels,
            samplers: animation_samplers,
            extensions: Default::default(),
            extras: Default::default(),
        });
    }

    b.pack()
}

fn build_node(
    b: &mut GlbBuilder,
    vfs: &MiniFs,
    model_dir: &Path,
    node: &CvdModelNode,
    channels: &mut Vec<Channel>,
    samplers: &mut Vec<Sampler>,
) -> gltf_json::Index<Node> {
    let mut gltf_node = Node::default();
    let mut mesh_idx: Option<gltf_json::Index<Mesh>> = None;

    if let Some(model) = &node.model {
        // Static node scale comes from `scale_factor`. (CVD uses this
        // independently of any scale keyframes — keyframes overlay
        // animated changes on top of the rest pose.)
        if (model.scale_factor - 1.0).abs() > f32::EPSILON {
            gltf_node.scale = Some([model.scale_factor; 3]);
        }

        if !model.mesh.frames.is_empty() && !model.mesh.materials.is_empty() {
            mesh_idx = Some(build_mesh(b, vfs, model_dir, &model.mesh));
        }
    }

    gltf_node.mesh = mesh_idx;

    // Reserve node index *now* so children can reference it; but glTF
    // nodes use `children` array of *indices*, and indices in
    // `gltf_json::Root` are assigned at push time. We therefore push
    // the placeholder node first, then update its children after
    // building them — easier than threading mutable index reservation.
    let node_idx = b.root.push(gltf_node);

    // Recurse into children.
    if let Some(children) = &node.children {
        let mut child_indices = Vec::with_capacity(children.len());
        for child in children {
            child_indices.push(build_node(b, vfs, model_dir, child, channels, samplers));
        }
        b.root.nodes[node_idx.value()].children = Some(child_indices);
    }

    // Wire TRS animation channels for this node, if any.
    if let Some(model) = &node.model {
        if let Some(pkf) = &model.position_keyframes {
            push_translation_anim(b, node_idx, pkf, channels, samplers);
        }
        if let Some(rkf) = &model.rotation_keyframes {
            push_rotation_anim(b, node_idx, rkf, channels, samplers);
        }
        if let Some(skf) = &model.scale_keyframes {
            push_scale_anim(b, node_idx, skf, channels, samplers);
        }
    }

    node_idx
}

fn build_mesh(
    b: &mut GlbBuilder,
    vfs: &MiniFs,
    model_dir: &Path,
    mesh: &CvdMesh,
) -> gltf_json::Index<Mesh> {
    let vertex_count = mesh.vertex_count as usize;
    let frame_count = mesh.frames.len();

    // Positions / UVs from the base frame, shared across all material
    // groups within this mesh.
    let base_positions: Vec<[f32; 3]> = mesh.frames[0]
        .iter()
        .map(|v| [v.position.x, v.position.y, v.position.z])
        .collect();
    let uvs: Vec<[f32; 2]> = mesh.frames[0]
        .iter()
        .map(|v| [v.tex_coord.x, v.tex_coord.y])
        .collect();
    let normals: Vec<[f32; 3]> = mesh.frames[0]
        .iter()
        .map(|v| [v.normal.x, v.normal.y, v.normal.z])
        .collect();

    let pos_acc = b.push_f32_accessor(&flatten_vec3(&base_positions), AccType::Vec3, true);
    let uv_acc = b.push_f32_accessor(&flatten_vec2(&uvs), AccType::Vec2, false);
    let nrm_acc = b.push_f32_accessor(&flatten_vec3(&normals), AccType::Vec3, false);

    // Per-frame morph targets (POSITION deltas vs frame 0).
    let mut morph_targets: Vec<MorphTarget> = Vec::new();
    for k in 1..frame_count {
        let deltas: Vec<[f32; 3]> = (0..vertex_count)
            .map(|i| {
                let p = &mesh.frames[k][i].position;
                let b0 = &base_positions[i];
                [p.x - b0[0], p.y - b0[1], p.z - b0[2]]
            })
            .collect();
        let acc = b.push_f32_accessor(&flatten_vec3(&deltas), AccType::Vec3, true);
        morph_targets.push(MorphTarget {
            positions: Some(acc),
            normals: None,
            tangents: None,
        });
    }

    let mut primitives = Vec::with_capacity(mesh.materials.len());
    for mat in &mesh.materials {
        let Some(tris) = &mat.triangles else { continue };
        if tris.is_empty() {
            continue;
        }
        let indices: Vec<u32> = tris
            .iter()
            .flat_map(|t| t.indices.iter().map(|i| *i as u32))
            .collect();
        let idx_acc = b.push_u32_indices(&indices);

        let material_idx = build_material(b, vfs, model_dir, Some(&mat.texture_name));

        let mut attributes = BTreeMap::new();
        attributes.insert(Checked::Valid(Semantic::Positions), pos_acc);
        attributes.insert(Checked::Valid(Semantic::Normals), nrm_acc);
        attributes.insert(Checked::Valid(Semantic::TexCoords(0)), uv_acc);

        primitives.push(Primitive {
            attributes,
            indices: Some(idx_acc),
            material: Some(material_idx),
            mode: Checked::Valid(gltf_json::mesh::Mode::Triangles),
            targets: if morph_targets.is_empty() {
                None
            } else {
                Some(morph_targets.clone())
            },
            extensions: Default::default(),
            extras: Default::default(),
        });
    }

    let weights = if morph_targets.is_empty() {
        None
    } else {
        Some(vec![0.0; morph_targets.len()])
    };
    b.root.push(Mesh {
        primitives,
        weights,
        extensions: Default::default(),
        extras: Default::default(),
    })
}

fn push_translation_anim(
    b: &mut GlbBuilder,
    node_idx: gltf_json::Index<Node>,
    pkf: &CvdPositionKeyFrames,
    channels: &mut Vec<Channel>,
    samplers: &mut Vec<Sampler>,
) {
    if pkf.frames.is_empty() {
        return;
    }
    let times: Vec<f32> = pkf.frames.iter().map(|f| f.timestamp).collect();
    let values: Vec<f32> = pkf
        .frames
        .iter()
        .flat_map(|f| [f.position.x, f.position.y, f.position.z])
        .collect();
    push_anim_channel(
        b,
        node_idx,
        Property::Translation,
        &times,
        &values,
        AccType::Vec3,
        channels,
        samplers,
    );
}

fn push_rotation_anim(
    b: &mut GlbBuilder,
    node_idx: gltf_json::Index<Node>,
    rkf: &CvdRotationKeyFrames,
    channels: &mut Vec<Channel>,
    samplers: &mut Vec<Sampler>,
) {
    if rkf.frames.is_empty() {
        return;
    }
    let times: Vec<f32> = rkf.frames.iter().map(|f| f.timestamp).collect();
    let values: Vec<f32> = rkf
        .frames
        .iter()
        .flat_map(|f| {
            // glTF stores quaternions as (x, y, z, w).
            let q = f.quaternion;
            [q.x, q.y, q.z, q.w]
        })
        .collect();
    push_anim_channel(
        b,
        node_idx,
        Property::Rotation,
        &times,
        &values,
        AccType::Vec4,
        channels,
        samplers,
    );
}

fn push_scale_anim(
    b: &mut GlbBuilder,
    node_idx: gltf_json::Index<Node>,
    skf: &CvdScaleKeyFrames,
    channels: &mut Vec<Channel>,
    samplers: &mut Vec<Sampler>,
) {
    if skf.frames.is_empty() {
        return;
    }
    // CVD scale keyframes also carry a "scale orientation" quaternion,
    // but glTF only models a Vec3 scale. We export the magnitude
    // component and skip the orientation pivot in v1 — exporting a
    // pre-rotated/scaled/un-rotated chain would require synthetic
    // helper nodes per part.
    let times: Vec<f32> = skf.frames.iter().map(|f| f.timestamp).collect();
    let values: Vec<f32> = skf
        .frames
        .iter()
        .flat_map(|f| [f.scale.x, f.scale.y, f.scale.z])
        .collect();
    push_anim_channel(
        b,
        node_idx,
        Property::Scale,
        &times,
        &values,
        AccType::Vec3,
        channels,
        samplers,
    );
}

fn push_anim_channel(
    b: &mut GlbBuilder,
    node_idx: gltf_json::Index<Node>,
    property: Property,
    times: &[f32],
    values: &[f32],
    output_ty: AccType,
    channels: &mut Vec<Channel>,
    samplers: &mut Vec<Sampler>,
) {
    let input_acc = b.push_f32_accessor(times, AccType::Scalar, true);
    let output_acc = b.push_f32_accessor(values, output_ty, false);

    let sampler_index = gltf_json::Index::new(samplers.len() as u32);
    samplers.push(Sampler {
        input: input_acc,
        interpolation: Checked::Valid(Interpolation::Linear),
        output: output_acc,
        extensions: Default::default(),
        extras: Default::default(),
    });
    channels.push(Channel {
        sampler: sampler_index,
        target: Target {
            node: node_idx,
            path: Checked::Valid(property),
            extensions: Default::default(),
            extras: Default::default(),
        },
        extensions: Default::default(),
        extras: Default::default(),
    });
}
