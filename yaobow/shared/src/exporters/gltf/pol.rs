//! `pol` (static, possibly multi-mesh / multi-material) → glTF.
//!
//! Each `PolMesh` becomes one glTF `Mesh` with one `Primitive` per
//! `PolMaterialInfo` (group of triangles that share a material). Each
//! `PolMesh` gets its own scene node. Vertex attributes come straight
//! out of `PolVertex`, mirroring the OBJ exporter's selection rules:
//!
//! * Always emit `tex_coord` (the primary UV set) as `TEXCOORD_0`.
//!   In PAL3 lightmapped meshes the secondary `tex_coord2` set is the
//!   *lightmap* UV (see `vulkan/shaders/lightmap_texture.frag`:
//!   `texSampler[0]`=lightmap sampled with `fragTexCoord2`,
//!   `texSampler[1]`=diffuse sampled with `fragTexCoord`). Since the
//!   glTF exporter only embeds the diffuse texture, pairing it with
//!   `tex_coord2` would sample the diffuse at lightmap coordinates and
//!   produce visibly wrong UVs. glTF specifies a top-left UV origin
//!   (same as PAL3/D3D) so V is passed through unchanged — no `1 - v`
//!   flip like the OBJ exporter needs for Blender.
//! * `material_info.texture_names.last()` selects the diffuse
//!   texture — for a 2-texture lightmap material the file order is
//!   `[lightmap, diffuse]`, so `.last()` is the diffuse.

use std::path::Path;

use fileformats::pol::{PolFile, PolMesh};
use gltf_json::accessor::Type as AccType;
use gltf_json::mesh::{Primitive, Semantic};
use gltf_json::validation::Checked;
use gltf_json::{Mesh, Node, Scene};
use mini_fs::MiniFs;

use super::glb::GlbBuilder;
use super::mv3::{build_material, flatten_vec2, flatten_vec3};

pub fn export_pol_to_glb(
    pol: &PolFile,
    vfs: &MiniFs,
    model_path: &Path,
) -> anyhow::Result<Vec<u8>> {
    let mut b = GlbBuilder::new();
    let model_dir = model_path.parent().unwrap_or_else(|| Path::new(""));

    let mut root_children = Vec::new();

    for pol_mesh in &pol.meshes {
        let (positions, uvs) = extract_attrs(pol_mesh);
        let pos_acc = b.push_f32_accessor(&flatten_vec3(&positions), AccType::Vec3, true);
        let uv_acc = b.push_f32_accessor(&flatten_vec2(&uvs), AccType::Vec2, false);

        let mut primitives = Vec::with_capacity(pol_mesh.material_info.len());
        for mat_info in &pol_mesh.material_info {
            let indices: Vec<u32> = mat_info
                .triangles
                .iter()
                .flat_map(|t| t.indices.iter().map(|i| *i as u32))
                .collect();
            if indices.is_empty() {
                continue;
            }
            let index_acc = b.push_u32_indices(&indices);

            let texture_name = mat_info
                .texture_names
                .last()
                .and_then(|n| n.as_str().ok());
            let material_idx = build_material(&mut b, vfs, model_dir, texture_name.as_deref());

            let mut attributes = std::collections::BTreeMap::new();
            attributes.insert(Checked::Valid(Semantic::Positions), pos_acc);
            attributes.insert(Checked::Valid(Semantic::TexCoords(0)), uv_acc);

            primitives.push(Primitive {
                attributes,
                indices: Some(index_acc),
                material: Some(material_idx),
                mode: Checked::Valid(gltf_json::mesh::Mode::Triangles),
                targets: None,
                extensions: Default::default(),
                extras: Default::default(),
            });
        }

        if primitives.is_empty() {
            continue;
        }
        let mesh_idx = b.root.push(Mesh {
            primitives,
            weights: None,
            extensions: Default::default(),
            extras: Default::default(),
        });
        let node_idx = b.root.push(Node {
            mesh: Some(mesh_idx),
            ..Node::default()
        });
        root_children.push(node_idx);
    }

    let scene_idx = b.root.push(Scene {
        nodes: root_children,
        extensions: Default::default(),
        extras: Default::default(),
    });
    b.root.scene = Some(scene_idx);

    b.pack()
}

fn extract_attrs(mesh: &PolMesh) -> (Vec<[f32; 3]>, Vec<[f32; 2]>) {
    let positions: Vec<[f32; 3]> = mesh
        .vertices
        .iter()
        .map(|v| [v.position.x, v.position.y, v.position.z])
        .collect();
    let uvs: Vec<[f32; 2]> = mesh
        .vertices
        .iter()
        .map(|v| [v.tex_coord.u, v.tex_coord.v])
        .collect();
    (positions, uvs)
}
