//! Minimal in-memory glTF/GLB builder for synthetic importer tests.
//!
//! Hand-packs a binary buffer plus a JSON document into a valid `.glb`
//! byte blob (via [`gltf::binary::Glb::to_vec`], the same helper the
//! `gltf` crate itself uses to *write* GLBs) and feeds it through
//! [`gltf::Gltf::from_slice`] — the exact parsing path
//! [`super::loader::load_gltf_scene`] uses for a real file — so tests
//! exercise real glTF parsing/validation without touching the filesystem
//! or depending on any external authoring tool.

#![cfg(test)]
// This builder exposes a general-purpose set of glTF construction helpers
// for tests to pick from; not every helper is exercised by the current
// test suite.
#![allow(dead_code)]

use std::borrow::Cow;

use gltf::binary::{Glb, Header};
use serde_json::{Value, json};

/// Accumulates accessors/bufferViews/binary data plus top-level document
/// arrays (nodes/meshes/materials/images/textures/animations) for one
/// synthetic glTF document.
#[derive(Default)]
pub struct SceneBuilder {
    bin: Vec<u8>,
    accessors: Vec<Value>,
    buffer_views: Vec<Value>,
    nodes: Vec<Value>,
    meshes: Vec<Value>,
    materials: Vec<Value>,
    images: Vec<Value>,
    textures: Vec<Value>,
    animations: Vec<Value>,
    asset_extras: Option<Value>,
}

/// glTF component type codes (see the glTF 2.0 spec's accessor table).
pub const COMPONENT_FLOAT: u32 = 5126;
pub const COMPONENT_U16: u32 = 5123;
pub const COMPONENT_U32: u32 = 5125;

const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;

impl SceneBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    fn push_aligned(&mut self, bytes: &[u8]) -> (usize, usize) {
        while self.bin.len() % 4 != 0 {
            self.bin.push(0);
        }
        let offset = self.bin.len();
        self.bin.extend_from_slice(bytes);
        (offset, bytes.len())
    }

    fn push_buffer_view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        let (offset, length) = self.push_aligned(bytes);
        let index = self.buffer_views.len();
        let mut bv = json!({ "buffer": 0, "byteOffset": offset, "byteLength": length });
        if let Some(t) = target {
            bv["target"] = json!(t);
        }
        self.buffer_views.push(bv);
        index
    }

    /// Adds a `VEC3` `f32` accessor (used for `POSITION`/`NORMAL`/morph
    /// target deltas/TRS translation-scale outputs). `min`/`max` are
    /// always emitted (required by the glTF spec for `POSITION`
    /// accessors, harmless elsewhere).
    pub fn add_vec3(&mut self, data: &[[f32; 3]], as_vertex_attribute: bool) -> usize {
        let bytes: Vec<u8> = data
            .iter()
            .flatten()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let target = as_vertex_attribute.then_some(ARRAY_BUFFER);
        let bv = self.push_buffer_view(&bytes, target);
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for p in data {
            for i in 0..3 {
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i]);
            }
        }
        let index = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": bv,
            "componentType": COMPONENT_FLOAT,
            "count": data.len(),
            "type": "VEC3",
            "min": min,
            "max": max,
        }));
        index
    }

    /// Adds a `VEC2` `f32` accessor (`TEXCOORD_0`).
    pub fn add_vec2(&mut self, data: &[[f32; 2]]) -> usize {
        let bytes: Vec<u8> = data
            .iter()
            .flatten()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let bv = self.push_buffer_view(&bytes, Some(ARRAY_BUFFER));
        let index = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": bv,
            "componentType": COMPONENT_FLOAT,
            "count": data.len(),
            "type": "VEC2",
        }));
        index
    }

    /// Adds a `VEC4` `f32` accessor (rotation TRS output).
    pub fn add_vec4(&mut self, data: &[[f32; 4]]) -> usize {
        let bytes: Vec<u8> = data
            .iter()
            .flatten()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let bv = self.push_buffer_view(&bytes, None);
        let index = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": bv,
            "componentType": COMPONENT_FLOAT,
            "count": data.len(),
            "type": "VEC4",
        }));
        index
    }

    /// Adds a `SCALAR` `f32` accessor (animation sampler `input` times,
    /// or morph `weights` sampler output).
    pub fn add_scalar_f32(&mut self, data: &[f32]) -> usize {
        let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
        let bv = self.push_buffer_view(&bytes, None);
        let index = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": bv,
            "componentType": COMPONENT_FLOAT,
            "count": data.len(),
            "type": "SCALAR",
        }));
        index
    }

    /// Adds a `u16` `SCALAR` index accessor.
    pub fn add_indices_u16(&mut self, data: &[u16]) -> usize {
        let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
        let bv = self.push_buffer_view(&bytes, Some(ELEMENT_ARRAY_BUFFER));
        let index = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": bv,
            "componentType": COMPONENT_U16,
            "count": data.len(),
            "type": "SCALAR",
        }));
        index
    }

    /// Adds a mesh with a single triangle-list primitive built from
    /// `positions`/`uv0`/`indices` (required) and `normals` (optional).
    /// Returns the mesh index.
    pub fn add_triangle_mesh(
        &mut self,
        positions: &[[f32; 3]],
        normals: Option<&[[f32; 3]]>,
        uv0: &[[f32; 2]],
        indices: &[u16],
        material: Option<usize>,
        morph_position_deltas: &[Vec<[f32; 3]>],
    ) -> usize {
        let pos_acc = self.add_vec3(positions, true);
        let uv_acc = self.add_vec2(uv0);
        let idx_acc = self.add_indices_u16(indices);

        let mut attributes = json!({ "POSITION": pos_acc, "TEXCOORD_0": uv_acc });
        if let Some(normals) = normals {
            let n_acc = self.add_vec3(normals, true);
            attributes["NORMAL"] = json!(n_acc);
        }

        let mut primitive = json!({
            "attributes": attributes,
            "indices": idx_acc,
            "mode": 4, // TRIANGLES
        });
        if let Some(m) = material {
            primitive["material"] = json!(m);
        }
        if !morph_position_deltas.is_empty() {
            let targets: Vec<Value> = morph_position_deltas
                .iter()
                .map(|deltas| {
                    let acc = self.add_vec3(deltas, false);
                    json!({ "POSITION": acc })
                })
                .collect();
            primitive["targets"] = json!(targets);
        }

        let index = self.meshes.len();
        self.meshes.push(json!({ "primitives": [primitive] }));
        index
    }

    /// Adds a mesh with a single primitive using an explicit topology
    /// `mode` (see the glTF spec's `Mesh.Primitive.mode` table; `1` =
    /// `LINES`), for exercising [`ImportError::UnsupportedTopology`].
    pub fn add_mesh_with_mode(
        &mut self,
        positions: &[[f32; 3]],
        uv0: &[[f32; 2]],
        indices: &[u16],
        mode: u32,
    ) -> usize {
        let pos_acc = self.add_vec3(positions, true);
        let uv_acc = self.add_vec2(uv0);
        let idx_acc = self.add_indices_u16(indices);
        let primitive = json!({
            "attributes": { "POSITION": pos_acc, "TEXCOORD_0": uv_acc },
            "indices": idx_acc,
            "mode": mode,
        });
        let index = self.meshes.len();
        self.meshes.push(json!({ "primitives": [primitive] }));
        index
    }

    /// Adds a node. `mesh`/`children`/`translation`/`rotation`/`scale`
    /// are all optional (glTF defaults: identity transform, no mesh, no
    /// children).
    #[allow(clippy::too_many_arguments)]
    pub fn add_node(
        &mut self,
        mesh: Option<usize>,
        children: &[usize],
        translation: Option<[f32; 3]>,
        rotation: Option<[f32; 4]>,
        scale: Option<[f32; 3]>,
    ) -> usize {
        let mut node = json!({});
        if let Some(m) = mesh {
            node["mesh"] = json!(m);
        }
        if !children.is_empty() {
            node["children"] = json!(children);
        }
        if let Some(t) = translation {
            node["translation"] = json!(t);
        }
        if let Some(r) = rotation {
            node["rotation"] = json!(r);
        }
        if let Some(s) = scale {
            node["scale"] = json!(s);
        }
        let index = self.nodes.len();
        self.nodes.push(node);
        index
    }

    /// Adds an image sourced from a relative file `uri` (no actual file
    /// needs to exist on disk for [`load_gltf_scene_from`], since only
    /// the *name* is read, never decoded pixels).
    pub fn add_image_uri(&mut self, uri: &str) -> usize {
        let index = self.images.len();
        self.images.push(json!({ "uri": uri }));
        index
    }

    /// Adds a `bufferView`-embedded image (no on-disk name at all) —
    /// exercises [`super::loader`]'s placeholder-name/diagnostic path.
    pub fn add_image_embedded(&mut self, bytes: &[u8], mime_type: &str) -> usize {
        let bv = self.push_buffer_view(bytes, None);
        let index = self.images.len();
        self.images
            .push(json!({ "bufferView": bv, "mimeType": mime_type }));
        index
    }

    pub fn add_texture(&mut self, image: usize) -> usize {
        let index = self.textures.len();
        self.textures.push(json!({ "source": image }));
        index
    }

    /// Adds a material with an (optional) base-color texture and alpha
    /// mode.
    pub fn add_material(&mut self, texture: Option<usize>, alpha_blend: bool) -> usize {
        let mut material = json!({
            "alphaMode": if alpha_blend { "BLEND" } else { "OPAQUE" },
        });
        if let Some(t) = texture {
            material["pbrMetallicRoughness"] = json!({ "baseColorTexture": { "index": t } });
        }
        let index = self.materials.len();
        self.materials.push(material);
        index
    }

    /// Adds a `weights` animation channel/sampler targeting `node`.
    pub fn add_weights_animation(&mut self, node: usize, times: &[f32], weights: &[f32]) -> usize {
        let input = self.add_scalar_f32(times);
        let output = self.add_scalar_f32(weights);
        let sampler = json!({ "input": input, "output": output, "interpolation": "LINEAR" });
        let channel = json!({
            "sampler": 0,
            "target": { "node": node, "path": "weights" },
        });
        let index = self.animations.len();
        self.animations
            .push(json!({ "samplers": [sampler], "channels": [channel] }));
        index
    }

    /// Adds a TRS animation channel/sampler targeting `node`.
    /// `path` is `"translation"`/`"rotation"`/`"scale"`; `values` holds
    /// one `[f32; N]` array (flattened) per keyframe (3 components for
    /// translation/scale, 4 for rotation) matching `times.len()`.
    pub fn add_trs_animation(
        &mut self,
        node: usize,
        path: &str,
        times: &[f32],
        values_flat: &[f32],
        interpolation: &str,
    ) -> usize {
        let input = self.add_scalar_f32(times);
        let bytes: Vec<u8> = values_flat.iter().flat_map(|f| f.to_le_bytes()).collect();
        let bv = self.push_buffer_view(&bytes, None);
        let component_count = values_flat.len() / times.len();
        let ty = if component_count == 4 { "VEC4" } else { "VEC3" };
        let output_index = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": bv,
            "componentType": COMPONENT_FLOAT,
            "count": times.len(),
            "type": ty,
        }));
        let sampler =
            json!({ "input": input, "output": output_index, "interpolation": interpolation });
        let channel = json!({
            "sampler": 0,
            "target": { "node": node, "path": path },
        });
        let index = self.animations.len();
        self.animations
            .push(json!({ "samplers": [sampler], "channels": [channel] }));
        index
    }

    pub fn set_asset_extras(&mut self, extras: Value) {
        self.asset_extras = Some(extras);
    }

    /// Assembles the final glTF JSON document and binary buffer, then
    /// parses them into a [`gltf::Gltf`] via a real (in-memory) GLB byte
    /// blob — see the module docs for why this route is used instead of
    /// constructing `gltf::Document` fields directly.
    pub fn parse(self, scene_roots: &[usize]) -> gltf::Gltf {
        let mut asset = json!({ "version": "2.0" });
        if let Some(extras) = &self.asset_extras {
            asset["extras"] = extras.clone();
        }
        let root = json!({
            "asset": asset,
            "buffers": [{ "byteLength": self.bin.len() }],
            "bufferViews": self.buffer_views,
            "accessors": self.accessors,
            "meshes": self.meshes,
            "nodes": self.nodes,
            "materials": self.materials,
            "images": self.images,
            "textures": self.textures,
            "animations": self.animations,
            "scenes": [{ "nodes": scene_roots }],
            "scene": 0,
        });

        let json_bytes = serde_json::to_vec(&root).expect("serializing synthetic glTF JSON");
        let glb = Glb {
            header: Header {
                magic: *b"glTF",
                version: 2,
                length: 0, // recomputed by `to_vec`/`to_writer`.
            },
            json: Cow::Owned(json_bytes),
            bin: Some(Cow::Owned(self.bin)),
        };
        let bytes = glb.to_vec().expect("assembling synthetic GLB");
        gltf::Gltf::from_slice(&bytes).expect("parsing synthetic GLB")
    }
}
