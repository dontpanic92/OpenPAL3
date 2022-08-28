use crate::ComObject_PolModel;
use crate::{loaders::pol_loader::*, material::LightMapMaterialDef};
use mini_fs::{MiniFs, StoreExt};
use radiance::interfaces::IComponentImpl;
use radiance::math::{Vec2, Vec3};
use radiance::rendering::{
    ComponentFactory, MaterialDef, RenderingComponent, SimpleMaterialDef, VertexBuffer,
    VertexComponents,
};
use radiance::scene::{CoreEntity, EntityExtension};
use std::any::TypeId;
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

pub struct PolModelEntity {}

impl PolModelEntity {
    pub fn new<P: AsRef<Path>>(
        component_factory: &Rc<dyn ComponentFactory>,
        vfs: &MiniFs,
        path: P,
        name: String,
        visible: bool,
    ) -> CoreEntity<Self> {
        let mut entity = CoreEntity::new(Self {}, name, visible);
        entity.add_component2(crosscom::ComRc::from_object(PolModel::new(
            component_factory,
            vfs,
            path,
        )));
        entity
    }
}

impl EntityExtension for PolModelEntity {}

pub struct PolModel {
    component_factory: Rc<dyn ComponentFactory>,
    meshes: Vec<PolMesh>,
}

ComObject_PolModel!(super::PolModel);

impl IComponentImpl for PolModel {
    fn on_loading(&self, entity: &mut dyn radiance::scene::Entity) -> crosscom::Void {
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
        entity.add_component(TypeId::of::<RenderingComponent>(), Box::new(component));
    }
}

impl PolModel {
    pub fn new<P: AsRef<Path>>(
        component_factory: &Rc<dyn ComponentFactory>,
        vfs: &MiniFs,
        path: P,
    ) -> Self {
        let pol = pol_load_from_file(vfs, path.as_ref()).unwrap();
        let mut meshes = vec![];
        for mesh in &pol.meshes {
            for material in &mesh.material_info {
                let mesh = PolMesh::new(
                    &mesh.vertices,
                    &material.triangles,
                    Self::load_material(&material, vfs, path.as_ref()),
                    material.use_alpha,
                );

                meshes.push(mesh);
            }
        }

        PolModel {
            component_factory: component_factory.clone(),
            meshes,
        }
    }

    fn load_material<P: AsRef<Path>>(
        material: &PolMaterialInfo,
        vfs: &MiniFs,
        path: P,
    ) -> MaterialDef {
        let texture_paths: Vec<PathBuf> = material
            .texture_names
            .iter()
            .map(|name| {
                name.split_terminator('.')
                    .next()
                    .and_then(|n| Some(n.to_owned() + ".dds"))
                    .and_then(|dds_name| {
                        let mut texture_path = path.as_ref().to_owned();
                        texture_path.pop();
                        texture_path.push(dds_name);
                        if !vfs.open(&texture_path).is_ok() {
                            texture_path.pop();
                            texture_path.push(name);
                        }

                        Some(texture_path)
                    })
                    .or(Some(PathBuf::from(name)))
                    .unwrap()
            })
            .collect();

        if texture_paths.len() == 1 {
            SimpleMaterialDef::create(
                texture_paths[0].to_str().unwrap(),
                |name| vfs.open(name).ok(),
                material.use_alpha != 0,
            )
        } else {
            let textures: Vec<_> = texture_paths.iter().map(|p| p.to_str().unwrap()).collect();
            LightMapMaterialDef::create(
                textures,
                |name| {
                    PathBuf::from(name)
                        .file_stem()
                        .and_then(|_| vfs.open(name).ok())
                },
                material.use_alpha != 0,
            )
        }
    }
}

struct PolMesh {
    material: MaterialDef,
    vertices: VertexBuffer,
    indices: Vec<u32>,
}

impl PolMesh {
    pub fn new(
        all_vertices: &Vec<PolVertex>,
        triangles: &[PolTriangle],
        material: MaterialDef,
        has_alpha: u32,
    ) -> Self {
        let components = if material.textures().len() == 1 {
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
        for t in triangles {
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

        Self {
            material,
            vertices,
            indices,
        }
    }
}
