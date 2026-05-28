//! `mv3` (animated role models) → glTF.
//!
//! A PAL3 mv3 file can carry **multiple models** (e.g. body + head),
//! each with its own texture and its own set of meshes. The exporter
//! emits one glTF `Mesh` (= one scene node) per `(model, mesh)` pair
//! so head textures don't end up painted on the body.
//!
//! Each remaining frame of a model becomes a **morph target** carrying
//! `POSITION` deltas relative to frame 0. A single STEP-interpolated
//! `weights` animation channel per node cycles a one-hot weight vector
//! across the frames, so the result snaps to the original per-frame
//! timing the same way the engine does. (`Animation.sampler.input` is
//! given in seconds derived from the engine's `4580 ticks/sec`
//! constant — see `create_animated_mesh_from_mv3`.)
//!
//! Vertex/UV expansion mirrors `create_geometry_frames`: each
//! `(position_index, texcoord_index)` pair becomes one glTF vertex,
//! deduped via a hash map. The engine flips V (`-v`) when uploading
//! mv3 texcoords to the GPU because the original PAL3 mv3 UVs were
//! authored with an OpenGL-style bottom-left origin; glTF (and our
//! re-encoded PNG textures) use a top-left origin, so we apply the
//! same `1 - v` flip here to match what the engine renders.

use std::collections::HashMap;
use std::path::Path;

use fileformats::mv3::{Mv3File, Mv3Mesh, Mv3Model};
use gltf_json::accessor::Type as AccType;
use gltf_json::animation::{Channel, Property, Sampler, Target};
use gltf_json::material::PbrBaseColorFactor;
use gltf_json::mesh::{MorphTarget, Primitive, Semantic};
use gltf_json::validation::Checked;
use gltf_json::{Material, Mesh, Node, Scene, Texture};
use mini_fs::MiniFs;

use super::glb::GlbBuilder;
use super::textures::embed_texture;

/// PAL3 mv3 timeline rate, matching `role_controller.rs` (`/ 4580.`).
const MV3_TICKS_PER_SECOND: f32 = 4580.0;

/// Engine vertex scale that maps Mv3 i16 coords to world units, matching
/// `create_geometry_frames`.
const MV3_VERTEX_SCALE: f32 = 0.01562;

pub fn export_mv3_to_glb(
    mv3: &Mv3File,
    vfs: &MiniFs,
    model_path: &Path,
) -> anyhow::Result<Vec<u8>> {
    if mv3.models.is_empty() {
        anyhow::bail!("mv3 has no models");
    }
    let mut b = GlbBuilder::new();

    let model_dir = model_path.parent().unwrap_or_else(|| Path::new(""));

    // Build one Mesh+Node per (model, mesh) pair so each part can
    // carry its own texture and its own morph-weights animation. PAL3
    // role mv3 files routinely split body + head into separate models
    // with separate textures; collapsing them onto a single primitive
    // would paint the wrong texture (e.g. head atlas) onto every part.
    let mut root_children: Vec<gltf_json::Index<Node>> = Vec::new();
    let mut animation_channels: Vec<Channel> = Vec::new();
    let mut animation_samplers: Vec<Sampler> = Vec::new();
    let mut shared_time_acc: Option<gltf_json::Index<gltf_json::Accessor>> = None;

    for (model_index, model) in mv3.models.iter().enumerate() {
        let frame_count = model.frame_count as usize;
        if frame_count == 0 || model.meshes.is_empty() {
            continue;
        }

        // Texture selection mirrors `create_animated_mesh_from_mv3`:
        // pick textures[model_index] if it exists, else fall back to
        // textures[0]. Empty texture list → untextured material.
        let texture_index = if (model_index as u32) < mv3.texture_count {
            model_index
        } else {
            0
        };
        let texture_name = mv3
            .textures
            .get(texture_index)
            .and_then(|t| t.names.get(0))
            .and_then(|n| n.to_string().ok());
        let material_idx = build_material(&mut b, vfs, model_dir, texture_name.as_deref());

        for mesh_index in 0..model.mesh_count as usize {
            let mesh = &model.meshes[mesh_index];
            let (indices, expanded_per_frame, uvs) = expand_mv3_mesh(model, mesh);
            if uvs.is_empty() || indices.is_empty() {
                continue;
            }
            let vertex_count = uvs.len();

            let position_acc =
                b.push_f32_accessor(&flatten_vec3(&expanded_per_frame[0]), AccType::Vec3, true);
            let uv_acc = b.push_f32_accessor(&flatten_vec2(&uvs), AccType::Vec2, false);
            let indices_acc = b.push_u32_indices(&indices);

            let base = &expanded_per_frame[0];
            let mut morph_targets: Vec<MorphTarget> =
                Vec::with_capacity(frame_count.saturating_sub(1));
            for frame_index in 1..frame_count {
                let frame = &expanded_per_frame[frame_index];
                let deltas: Vec<[f32; 3]> = (0..vertex_count)
                    .map(|i| {
                        [
                            frame[i][0] - base[i][0],
                            frame[i][1] - base[i][1],
                            frame[i][2] - base[i][2],
                        ]
                    })
                    .collect();
                let acc = b.push_f32_accessor(&flatten_vec3(&deltas), AccType::Vec3, true);
                morph_targets.push(MorphTarget {
                    positions: Some(acc),
                    normals: None,
                    tangents: None,
                });
            }

            let mut attributes = std::collections::BTreeMap::new();
            attributes.insert(Checked::Valid(Semantic::Positions), position_acc);
            attributes.insert(Checked::Valid(Semantic::TexCoords(0)), uv_acc);

            let target_count = morph_targets.len();
            let primitive = Primitive {
                attributes,
                indices: Some(indices_acc),
                material: Some(material_idx),
                mode: Checked::Valid(gltf_json::mesh::Mode::Triangles),
                targets: if morph_targets.is_empty() {
                    None
                } else {
                    Some(morph_targets)
                },
                extensions: Default::default(),
                extras: Default::default(),
            };

            let mesh_idx = b.root.push(Mesh {
                primitives: vec![primitive],
                weights: if target_count == 0 {
                    None
                } else {
                    Some(vec![0.0; target_count])
                },
                extensions: Default::default(),
                extras: Default::default(),
            });
            let node_idx = b.root.push(Node {
                mesh: Some(mesh_idx),
                ..Node::default()
            });
            root_children.push(node_idx);

            if target_count > 0 {
                // Reuse one shared time accessor across every channel —
                // all (model, mesh) pairs in a PAL3 mv3 file share the
                // same per-frame timeline (`models[0].frames`).
                let time_acc = match shared_time_acc {
                    Some(acc) => acc,
                    None => {
                        let times: Vec<f32> = model
                            .frames
                            .iter()
                            .map(|f| f.timestamp as f32 / MV3_TICKS_PER_SECOND)
                            .collect();
                        let acc = b.push_f32_accessor(&times, AccType::Scalar, true);
                        shared_time_acc = Some(acc);
                        acc
                    }
                };
                let mut weights = vec![0.0f32; frame_count * target_count];
                for k in 1..frame_count {
                    weights[k * target_count + (k - 1)] = 1.0;
                }
                let output_acc = b.push_f32_accessor(&weights, AccType::Scalar, false);
                let sampler_idx = animation_samplers.len() as u32;
                animation_samplers.push(Sampler {
                    input: time_acc,
                    interpolation: Checked::Valid(gltf_json::animation::Interpolation::Step),
                    output: output_acc,
                    extensions: Default::default(),
                    extras: Default::default(),
                });
                animation_channels.push(Channel {
                    sampler: gltf_json::Index::new(sampler_idx),
                    target: Target {
                        node: node_idx,
                        path: Checked::Valid(Property::MorphTargetWeights),
                        extensions: Default::default(),
                        extras: Default::default(),
                    },
                    extensions: Default::default(),
                    extras: Default::default(),
                });
            }
        }
    }

    if root_children.is_empty() {
        anyhow::bail!("mv3 produced no exportable geometry");
    }

    let scene_idx = b.root.push(Scene {
        nodes: root_children,
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

fn expand_mv3_mesh(
    model: &Mv3Model,
    mesh: &Mv3Mesh,
) -> (Vec<u32>, Vec<Vec<[f32; 3]>>, Vec<[f32; 2]>) {
    let frame_count = model.frame_count as usize;
    let mut indices: Vec<u32> = Vec::new();
    let mut positions_per_frame: Vec<Vec<[f32; 3]>> = vec![Vec::new(); frame_count];
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut index_map: HashMap<(u16, u16), u32> = HashMap::new();

    for t in &mesh.triangles {
        for (&i, &j) in t.indices.iter().zip(&t.texcoord_indices) {
            let key = (i, j);
            let next_idx = index_map.len() as u32;
            let idx = *index_map.entry(key).or_insert_with(|| {
                for k in 0..frame_count {
                    let v = &model.frames[k].vertices[i as usize];
                    positions_per_frame[k].push([
                        v.x as f32 * MV3_VERTEX_SCALE,
                        v.y as f32 * MV3_VERTEX_SCALE,
                        v.z as f32 * MV3_VERTEX_SCALE,
                    ]);
                }
                // V flip: engine uploads `-v` (see
                // `create_geometry_frames` in role_controller.rs)
                // because mv3 UVs are authored with OpenGL bottom-left
                // origin. glTF + our re-encoded textures are top-left;
                // emit `1 - v` so the sampled image matches the engine
                // (and stays in [0, 1] for DCC tools, unlike a bare
                // `-v` that only happens to work under REPEAT wrap).
                let uv = if (j as u32) < model.texcoord_count {
                    let t = &model.texcoords[j as usize];
                    [t.u, 1.0 - t.v]
                } else {
                    [0.0, 0.0]
                };
                uvs.push(uv);
                next_idx
            });
            indices.push(idx);
        }
    }

    (indices, positions_per_frame, uvs)
}

// (per-mesh morph-weight animation channels are appended inline in
// `export_mv3_to_glb`; no separate helper is needed.)

pub(super) fn build_material(
    b: &mut GlbBuilder,
    vfs: &MiniFs,
    model_dir: &Path,
    texture_name: Option<&str>,
) -> gltf_json::Index<Material> {
    let base_color_texture = texture_name
        .and_then(|name| embed_texture(b, vfs, model_dir, name))
        .map(|image_idx| {
            let tex = b.root.push(Texture {
                sampler: None,
                source: image_idx,
                extensions: Default::default(),
                extras: Default::default(),
            });
            gltf_json::texture::Info {
                index: tex,
                tex_coord: 0,
                extensions: Default::default(),
                extras: Default::default(),
            }
        });

    let pbr = gltf_json::material::PbrMetallicRoughness {
        base_color_factor: PbrBaseColorFactor([1.0, 1.0, 1.0, 1.0]),
        base_color_texture,
        metallic_factor: gltf_json::material::StrengthFactor(0.0),
        roughness_factor: gltf_json::material::StrengthFactor(1.0),
        metallic_roughness_texture: None,
        extensions: Default::default(),
        extras: Default::default(),
    };

    b.root.push(Material {
        pbr_metallic_roughness: pbr,
        // PAL3 role textures rely on alpha cutout for hair/eye fringes;
        // mirror the runtime's BlendMode::AlphaTest default.
        alpha_mode: Checked::Valid(gltf_json::material::AlphaMode::Mask),
        alpha_cutoff: Some(gltf_json::material::AlphaCutoff(0.5)),
        double_sided: false,
        normal_texture: None,
        occlusion_texture: None,
        emissive_texture: None,
        emissive_factor: gltf_json::material::EmissiveFactor([0.0, 0.0, 0.0]),
        extensions: Default::default(),
        extras: Default::default(),
    })
}

pub(super) fn flatten_vec3(v: &[[f32; 3]]) -> Vec<f32> {
    v.iter().flat_map(|x| x.iter().copied()).collect()
}
pub(super) fn flatten_vec2(v: &[[f32; 2]]) -> Vec<f32> {
    v.iter().flat_map(|x| x.iter().copied()).collect()
}
