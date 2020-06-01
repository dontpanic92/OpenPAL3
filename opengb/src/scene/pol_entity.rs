use crate::loaders::pol_loader::*;
use crate::material::LightMapMaterial;
use radiance::math::{Vec2, Vec3};
use radiance::rendering::{RenderObject, SimpleMaterial, VertexBuffer, VertexComponents};
use radiance::scene::{CoreEntity, EntityExtension};
use std::path::PathBuf;

pub struct PolModelEntity {
    texture_paths: Vec<PathBuf>,
    vertices: VertexBuffer,
    indices: Vec<u32>,
    alpha_blending_needed: u32,
    // pol: PolFile,
}

impl PolModelEntity {
    pub fn new(all_vertices: &Vec<PolVertex>, material: &PolMaterialInfo, path: &str) -> Self {
        let texture_paths: Vec<PathBuf> = material
            .texture_names
            .iter()
            .map(|name| {
                name.split_terminator('.')
                    .next()
                    .and_then(|n| Some(n.to_owned() + ".dds"))
                    .and_then(|dds_name| {
                        let mut texture_path = PathBuf::from(path);
                        texture_path.pop();
                        texture_path.push(dds_name);
                        if !texture_path.exists() {
                            texture_path.pop();
                            texture_path.push(name);
                        }

                        Some(texture_path)
                    })
                    .or(Some(PathBuf::from(name)))
                    .unwrap()
            })
            .collect();

        let components = if texture_paths.len() == 1 {
            VertexComponents::POSITION | VertexComponents::TEXCOORD
        } else {
            VertexComponents::POSITION | VertexComponents::TEXCOORD | VertexComponents::TEXCOORD2
        };

        let mut index_map = std::collections::HashMap::new();
        let mut reversed_index = vec![];
        let mut get_new_index = |index: u16| -> u32 {
            if index_map.contains_key(&index) {
                index_map[&index]
            } else {
                let new_index = reversed_index.len() as u32;
                reversed_index.push(index as usize);
                index_map.insert(index, new_index);
                new_index
            }
        };

        let mut indices: Vec<u32> = vec![];
        for t in &material.triangles {
            indices.push(get_new_index(t.indices[0]));
            indices.push(get_new_index(t.indices[1]));
            indices.push(get_new_index(t.indices[2]));
        }

        let mut vertices = VertexBuffer::new(components, reversed_index.len());

        for i in 0..reversed_index.len() {
            let vert = &all_vertices[reversed_index[i]];
            vertices.set_data(
                i,
                Some(&Vec3::new(
                    vert.position.x,
                    vert.position.y,
                    vert.position.z,
                )),
                None,
                Some(&Vec2::new(vert.tex_coord.u, vert.tex_coord.v)),
                vert.tex_coord2
                    .as_ref()
                    .and_then(|tex_coord2| Some(Vec2::new(tex_coord2.u, tex_coord2.v)))
                    .as_ref(),
            );
        }

        PolModelEntity {
            texture_paths,
            vertices,
            indices,
            alpha_blending_needed: material.has_alpha as u32,
        }
    }

    pub fn alpha_blending_needed(&self) -> u32 {
        self.alpha_blending_needed
    }
}

impl EntityExtension for PolModelEntity {
    fn on_loading(self: &mut CoreEntity<Self>) {
        self.add_component(RenderObject::new_with_data(
            self.vertices.clone(),
            self.indices.clone(),
            if self.texture_paths.len() == 1 {
                Box::new(SimpleMaterial::new(&self.texture_paths[0]))
            } else {
                Box::new(LightMapMaterial::new(&self.texture_paths))
            },
        ));
    }
}
