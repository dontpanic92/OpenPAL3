use crate::loaders::cvd_loader::*;
use mini_fs::{MiniFs, StoreExt};
use radiance::math::{Vec2, Vec3};
use radiance::scene::{CoreEntity, EntityExtension};
use radiance::{
    rendering::{ComponentFactory, MaterialDef, SimpleMaterialDef, VertexBuffer, VertexComponents},
    scene::Entity,
};
use std::{path::Path, rc::Rc};

pub struct CvdModelEntity {
    component_factory: Rc<dyn ComponentFactory>,
    position_keyframes: Option<CvdPositionKeyFrames>,
    rotation_keyframes: Option<CvdRotationKeyFrames>,
    scale_keyframes: Option<CvdScaleKeyFrames>,
    meshes: Vec<CvdMesh>,
}

impl CvdModelEntity {
    pub fn create<P: AsRef<Path>>(
        component_factory: Rc<dyn ComponentFactory>,
        vfs: &MiniFs,
        path: P,
        name: String,
    ) -> CoreEntity<Self> {
        let cvd = cvd_load_from_file(vfs, path.as_ref()).unwrap();
        let mut entity = CoreEntity::new(
            Self {
                component_factory: component_factory.clone(),
                position_keyframes: None,
                rotation_keyframes: None,
                scale_keyframes: None,
                meshes: vec![],
            },
            name,
        );

        for (i, node) in cvd.models.iter().enumerate() {
            entity.attach(Box::new(Self::new_from_cvd_model_node(
                component_factory.clone(),
                vfs,
                path.as_ref(),
                node,
            )));
        }

        entity
    }

    fn new_from_cvd_model_node<P: AsRef<Path>>(
        component_factory: Rc<dyn ComponentFactory>,
        vfs: &MiniFs,
        path: P,
        node: &CvdModelNode,
    ) -> CoreEntity<Self> {
        let mut scale_factor = 1.;
        let mut position_keyframes = None;
        let mut rotation_keyframes = None;
        let mut scale_keyframes = None;
        let mut meshes = vec![];
        if let Some(model) = &node.model {
            position_keyframes = model.position_keyframes.clone();
            rotation_keyframes = model.rotation_keyframes.clone();
            scale_keyframes = model.scale_keyframes.clone();

            for material in &model.mesh.materials {
                if material.triangles.is_none() {
                    continue;
                }

                for v in &model.mesh.frames {
                    let mesh = CvdMesh::new(
                        v,
                        material,
                        Self::load_texture(material, vfs, path.as_ref()),
                    );
                    meshes.push(mesh);

                    // TODO: Support multiple frames
                    break;
                }
            }

            scale_factor = model.scale_factor;
        }

        let mut entity = CoreEntity::new(
            Self {
                component_factory: component_factory.clone(),
                position_keyframes,
                rotation_keyframes,
                scale_keyframes,
                meshes,
            },
            "cvd_obj".to_string(),
        );

        entity.setup_transform(scale_factor);

        if let Some(children) = &node.children {
            for child in children {
                entity.attach(Box::new(Self::new_from_cvd_model_node(
                    component_factory.clone(),
                    vfs,
                    path.as_ref(),
                    &child,
                )));
            }
        }

        entity
    }

    pub fn setup_transform(self: &mut CoreEntity<Self>, scale_factor: f32) {
        self.transform_mut()
            .scale_local(&Vec3::new(scale_factor, scale_factor, scale_factor));

        if let Some(p) = self
            .position_keyframes
            .as_ref()
            .and_then(|frame| frame.frames.get(0))
            .and_then(|f| Some(f.position))
        {
            self.transform_mut().translate_local(&p);
        }

        if let Some(q) = self
            .rotation_keyframes
            .as_ref()
            .and_then(|frame| frame.frames.get(0))
            .and_then(|f| Some(f.quaternion))
        {
            self.transform_mut().rotate_quaternion_local(&q);
        }

        if let Some(frame) = self
            .scale_keyframes
            .as_ref()
            .and_then(|frame| frame.frames.get(0))
        {
            let scale = frame.scale;
            let q2 = frame.quaternion;
            let mut q3 = q2;
            q3.inverse();

            self.transform_mut()
                .rotate_quaternion_local(&q2)
                .scale_local(&scale)
                .rotate_quaternion_local(&q3);
        }
    }

    fn load_texture<P: AsRef<Path>>(
        material: &CvdMaterial,
        vfs: &MiniFs,
        model_path: P,
    ) -> MaterialDef {
        let dds_name = material
            .texture_name
            .split_terminator('.')
            .next()
            .unwrap()
            .to_owned()
            + ".dds";
        let mut texture_path = model_path.as_ref().to_owned();
        texture_path.pop();
        texture_path.push(&dds_name);
        if !vfs.open(&texture_path).is_ok() {
            texture_path.pop();
            texture_path.push(&material.texture_name);
        }

        SimpleMaterialDef::create(vfs.open(texture_path).as_mut().ok(), false)
    }
}

impl EntityExtension for CvdModelEntity {
    fn on_loading(self: &mut CoreEntity<Self>) {
        let mut objects = vec![];
        for mesh in &self.meshes {
            let ro = self.component_factory.create_render_object(
                mesh.vertices.clone(),
                mesh.indices.clone(),
                &mesh.material,
                false,
            );

            objects.push(ro);
        }

        let component = self.component_factory.create_rendering_component(objects);
        self.add_component(Box::new(component));
    }
}

struct CvdMesh {
    material: MaterialDef,
    vertices: VertexBuffer,
    indices: Vec<u32>,
}

impl CvdMesh {
    pub fn new(
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

        CvdMesh {
            material,
            vertices,
            indices,
        }
    }
}
