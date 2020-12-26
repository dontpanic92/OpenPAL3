use crate::loaders::cvd_loader::*;
use image::RgbaImage;
use radiance::rendering::{
    ComponentFactory, MaterialDef, SimpleMaterialDef, VertexBuffer, VertexComponents,
};
use radiance::scene::{CoreEntity, EntityExtension};
use radiance::{
    math::{Vec2, Vec3},
    rendering::RenderObject,
};
use std::rc::Rc;

pub struct CvdModelEntity {
    component_factory: Rc<dyn ComponentFactory>,
    material: MaterialDef,
    vertices: VertexBuffer,
    indices: Vec<u32>,
}

impl CvdModelEntity {
    pub fn new(
        component_factory: &Rc<dyn ComponentFactory>,
        all_vertices: &Vec<CvdVertex>,
        cvd_material: &CvdMaterial,
        material: MaterialDef,
    ) -> Self {
        let components =
            VertexComponents::POSITION /*| VertexComponents::NORMAL*/ | VertexComponents::TEXCOORD;

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
        for t in cvd_material.triangles.as_ref().unwrap() {
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
                Some(&Vec2::new(vert.tex_coord.x, vert.tex_coord.y)),
                None,
            );
        }

        CvdModelEntity {
            component_factory: component_factory.clone(),
            material,
            vertices,
            indices,
        }
    }
}

impl EntityExtension for CvdModelEntity {
    fn on_loading(self: &mut CoreEntity<Self>) {
        let ro = self.component_factory.create_render_object(
            self.vertices.clone(),
            self.indices.clone(),
            &self.material,
            false,
        );
        self.add_component::<Box<dyn RenderObject>>(Box::new(ro));
    }
}
