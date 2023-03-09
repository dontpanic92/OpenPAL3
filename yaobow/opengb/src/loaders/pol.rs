use crosscom::ComRc;
use fileformats::pol::{read_pol, PolMaterialInfo, PolTriangle, PolVertex};
use mini_fs::{MiniFs, StoreExt};
use radiance::comdef::{IEntity, IStaticMeshComponent};
use radiance::math::Vec3;
use radiance::rendering::{
    ComponentFactory, Geometry, MaterialDef, SimpleMaterialDef, StaticMeshComponent, TexCoord,
};
use radiance::scene::CoreEntity;
use std::io::BufReader;
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use crate::material::LightMapMaterialDef;

pub fn create_entity_from_pol_model<P: AsRef<Path>>(
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
    name: String,
    visible: bool,
) -> ComRc<IEntity> {
    let entity = CoreEntity::create(name, visible);
    let geometries = load_pol_model(vfs, path);
    let mesh_component =
        StaticMeshComponent::new(entity.clone(), geometries, component_factory.clone());
    entity.add_component(
        IStaticMeshComponent::uuid(),
        crosscom::ComRc::from_object(mesh_component),
    );
    entity
}

fn load_pol_model<P: AsRef<Path>>(vfs: &MiniFs, path: P) -> Vec<Geometry> {
    let mut reader = BufReader::new(vfs.open(&path).unwrap());
    let pol = read_pol(&mut reader).unwrap();
    let mut geometries = vec![];
    for mesh in &pol.meshes {
        for material in &mesh.material_info {
            let geometry = create_geometry(
                &mesh.vertices,
                &material.triangles,
                load_material(&material, vfs, path.as_ref()),
                material.use_alpha,
            );

            geometries.push(geometry);
        }
    }

    geometries
}

fn load_material<P: AsRef<Path>>(material: &PolMaterialInfo, vfs: &MiniFs, path: P) -> MaterialDef {
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

fn create_geometry(
    all_vertices: &Vec<PolVertex>,
    triangles: &[PolTriangle],
    material: MaterialDef,
    has_alpha: u32,
) -> Geometry {
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

    let mut vertices = vec![];

    let mut texcoord1 = vec![];
    let mut texcoord2 = vec![];

    for i in 0..reversed_index.len() {
        let vert = &all_vertices[reversed_index[i]];
        let v = Vec3::new(vert.position.x, vert.position.y, vert.position.z);
        vertices.push(v);
        texcoord1.push(TexCoord::new(vert.tex_coord.u, vert.tex_coord.v));

        if let Some(texcoord) = &vert.tex_coord2 {
            texcoord2.push(TexCoord::new(texcoord.u, texcoord.v));
        }
    }

    let texcoords = if texcoord2.is_empty() {
        vec![texcoord1]
    } else {
        vec![texcoord1, texcoord2]
    };

    Geometry::new(&vertices, None, &texcoords, indices, material, has_alpha)
}
