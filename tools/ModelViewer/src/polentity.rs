use opengb::loaders::polloader::*;
use radiance::scene::{Entity, CoreEntity, EntityCallbacks};
use radiance::rendering::{RenderObject, Vertex};
use radiance::math::{Vec2, Vec3};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct PolModelEntity {
    texture_path: PathBuf,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    pol: PolFile,
}

impl PolModelEntity {
    pub fn new(path: &String) -> Self {
        let pol = pol_load_from_file(&path).unwrap();
        let mesh: &PolMesh = &pol.meshes[0];

        let mut texture_path = PathBuf::from(path);
        texture_path.pop();
        texture_path.push("d011.dds");

        let mut vertices = vec![];
        for vert in &mesh.vertices {
            vertices.push(Vertex::new(
                Vec3::new(vert.position.x, vert.position.y, vert.position.z),
                Vec2::new(vert.tex_coord.u, vert.tex_coord.v),
            ))
        }

        let mut indices: Vec<u32> = vec![];
        for t in &mesh.triangles {
            indices.push(t.indices[0] as u32);
            indices.push(t.indices[1] as u32);
            indices.push(t.indices[2] as u32);
        }

        println!("{:?}", pol);

        PolModelEntity {
            texture_path,
            vertices,
            indices,
            pol,
        }
    }
}

impl EntityCallbacks for PolModelEntity {
    fn on_loading<T: EntityCallbacks>(&mut self, entity: &mut CoreEntity<T>) {
        entity.add_component(RenderObject::new_with_data(self.vertices.clone(), self.indices.clone(), &self.texture_path));
    }

    fn on_updating<T: EntityCallbacks>(&mut self, entity: &mut CoreEntity<T>, delta_sec: f32) {
        entity.transform_mut().rotate_local(&Vec3::new(0., 1., 0.), -0.2 * delta_sec * std::f32::consts::PI);
    }
}