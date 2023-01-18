use std::{
    io::{Cursor, Read},
    path::Path,
    rc::Rc,
};

use crosscom::ComRc;
use fileformats::dff::{self, read_dff};
use mini_fs::{MiniFs, StoreExt};
use radiance::{
    interfaces::{IEntity, IStaticMeshComponent},
    math::Vec3,
    rendering::{ComponentFactory, StaticMeshComponent},
    scene::CoreEntity,
};

pub fn create_entity_from_dff_model<P: AsRef<Path>>(
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
    name: String,
    visible: bool,
) -> ComRc<IEntity> {
    let entity = CoreEntity::create(name, visible);
    let geometries = load_dff_model(vfs, path);
    let mesh_component = StaticMeshComponent::new(geometries, component_factory.clone());
    entity.add_component(
        IStaticMeshComponent::uuid(),
        crosscom::ComRc::from_object(mesh_component),
    );
    entity
}

fn load_dff_model<P: AsRef<Path>>(vfs: &MiniFs, path: P) -> Vec<radiance::rendering::Geometry> {
    let mut data = vec![];
    let _ = vfs.open(&path).unwrap().read_to_end(&mut data).unwrap();
    let chunks = read_dff(&data).unwrap();
    if chunks.is_empty() {
        vec![]
    } else {
        let mut r_geometries = vec![];
        for atomic in &chunks[0].atomics {
            // let frame = &chunks[0].frames[atomic.frame as usize];
            let geometry = &chunks[0].geometries[atomic.geometry as usize];
            let r_geometry = create_geometry(geometry, vfs, &path);
            r_geometries.push(r_geometry);
        }

        r_geometries
    }
}

fn create_geometry<P: AsRef<Path>>(
    geometry: &dff::geometry::Geometry,
    vfs: &MiniFs,
    path: P,
) -> radiance::rendering::Geometry {
    let vertices = geometry.morph_targets[0].vertices.as_ref().unwrap();
    let normals = geometry.morph_targets[0].normals.as_ref().unwrap();

    let mut r_vertices = vec![];
    let mut r_normals = vec![];
    for i in 0..vertices.len() {
        r_vertices.push(Vec3::new(vertices[i].x, vertices[i].y, vertices[i].z));
        r_normals.push(Vec3::new(normals[i].x, normals[i].y, normals[i].z));
    }

    let mut indices = vec![];
    for t in &geometry.triangles {
        indices.push(t.index[0] as u32);
        indices.push(t.index[1] as u32);
        indices.push(t.index[2] as u32);
    }

    let (r_material, r_texcoords) = if let Some(texcoords) = geometry.texcoord_sets.as_ref() {
        let texcoords = texcoords
            .iter()
            .map(|t| {
                t.iter()
                    .map(|t| radiance::rendering::TexCoord::new(t.u, t.v))
                    .collect()
            })
            .collect();

        let material = &geometry.materials[geometry.triangles[0].material as usize];
        let tex_name = &material.texture.as_ref().unwrap().name;
        let tex_path = path
            .as_ref()
            .parent()
            .unwrap()
            .join(tex_name.to_string() + ".dds");

        (
            radiance::rendering::SimpleMaterialDef::create(
                tex_name,
                |_name| Some(vfs.open(&tex_path).unwrap()),
                true,
            ),
            texcoords,
        )
    } else {
        (
            radiance::rendering::SimpleMaterialDef::create(
                "n",
                |_name| None::<Cursor<&[u8]>>,
                true,
            ),
            vec![],
        )
    };

    radiance::rendering::Geometry::new(
        &r_vertices,
        Some(&r_normals),
        &r_texcoords,
        indices,
        r_material,
        1,
    )
}
