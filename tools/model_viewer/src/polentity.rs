use opengb::loaders::polloader::*;
use radiance::math::{Vec2, Vec3};
use radiance::rendering::{RenderObject, SimpleMaterial, VertexBuffer, VertexComponents};
use radiance::scene::{CoreEntity, Entity, EntityCallbacks};
use std::path::PathBuf;

pub struct PolModelEntity {
    texture_path: PathBuf,
    vertices: VertexBuffer,
    indices: Vec<u32>,
    // pol: PolFile,
}

impl PolModelEntity {
    pub fn new(mesh: &PolMesh, path: &str) -> Self {
        let texture_name: &str = &mesh.material_info[0].texture_names[0];
        let dds_name = texture_name
            .split_terminator('.')
            .next()
            .unwrap()
            .to_owned()
            + ".dds";
        let mut texture_path = PathBuf::from(path);
        texture_path.pop();
        texture_path.push(dds_name);

        let mut vertices = VertexBuffer::new(
            VertexComponents::POSITION | VertexComponents::TEXCOORD,
            mesh.vertices.len(),
        );

        for (i, vert) in mesh.vertices.iter().enumerate() {
            vertices.set_data(
                i,
                Some(&Vec3::new(
                    vert.position.x,
                    vert.position.y,
                    vert.position.z,
                )),
                None,
                Some(&Vec2::new(vert.tex_coord.u, vert.tex_coord.v)),
                None,
            );
        }

        let mut indices: Vec<u32> = vec![];
        for t in &mesh.material_info[0].triangles {
            indices.push(t.indices[0] as u32);
            indices.push(t.indices[1] as u32);
            indices.push(t.indices[2] as u32);
        }

        PolModelEntity {
            texture_path,
            vertices,
            indices,
        }
    }
}

impl EntityCallbacks for PolModelEntity {
    fn on_loading<T: EntityCallbacks>(&mut self, entity: &mut CoreEntity<T>) {
        entity.add_component(RenderObject::new_with_data(
            self.vertices.clone(),
            self.indices.clone(),
            Box::new(SimpleMaterial::new(&self.texture_path)),
        ));
    }

    fn on_updating<T: EntityCallbacks>(&mut self, entity: &mut CoreEntity<T>, delta_sec: f32) {
        entity.transform_mut().rotate_local(
            &Vec3::new(0., 1., 0.),
            -0.2 * delta_sec * std::f32::consts::PI,
        );
    }
}
