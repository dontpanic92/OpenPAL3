use std::{collections::HashMap, io::Read, path::Path, rc::Rc};

use crosscom::ComRc;
use fileformats::rwbs::{material::Material, read_dff, TexCoord, Triangle, Vec3f};
use mini_fs::{MiniFs, StoreExt};
use radiance::{
    comdef::{IEntity, IStaticMeshComponent},
    math::Vec3,
    rendering::{ComponentFactory, MaterialDef, StaticMeshComponent},
    scene::CoreEntity,
};

use super::TextureResolver;

pub fn create_entity_from_dff_model<P: AsRef<Path>>(
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
    name: String,
    visible: bool,
    texture_resolver: &dyn TextureResolver,
) -> ComRc<IEntity> {
    let entity = CoreEntity::create(name, visible);
    let geometries = load_dff_model(vfs, path, texture_resolver);
    let mesh_component =
        StaticMeshComponent::new(entity.clone(), geometries, component_factory.clone());
    entity.add_component(
        IStaticMeshComponent::uuid(),
        crosscom::ComRc::from_object(mesh_component),
    );
    entity
}

fn load_dff_model<P: AsRef<Path>>(
    vfs: &MiniFs,
    path: P,
    texture_resolver: &dyn TextureResolver,
) -> Vec<radiance::rendering::Geometry> {
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
            let mut r_geometry = create_geometry(geometry, vfs, &path, texture_resolver);
            r_geometries.append(&mut r_geometry);
        }

        r_geometries
    }
}

fn create_geometry<P: AsRef<Path>>(
    geometry: &fileformats::rwbs::geometry::Geometry,
    vfs: &MiniFs,
    path: P,
    texture_resolver: &dyn TextureResolver,
) -> Vec<radiance::rendering::Geometry> {
    if geometry.morph_targets.len() == 0 {
        return vec![];
    }

    if geometry.morph_targets[0].vertices.is_none() {
        return vec![];
    }

    let vertices = geometry.morph_targets[0].vertices.as_ref().unwrap();
    let normals = geometry.morph_targets[0].normals.as_ref();
    let triangles = &geometry.triangles;
    let texcoord_sets = if geometry.texcoord_sets.len() > 1 {
        vec![geometry.texcoord_sets[0].clone()]
    } else {
        geometry.texcoord_sets.clone()
    };
    let materials = &geometry.materials;

    create_geometry_internal(
        vertices,
        normals,
        triangles,
        &texcoord_sets,
        materials,
        vfs,
        path,
        texture_resolver,
    )
}

pub(crate) fn create_geometry_internal<P: AsRef<Path>>(
    vertices: &[Vec3f],
    normals: Option<&Vec<Vec3f>>,
    triangles: &[Triangle],
    texcoord_sets: &[Vec<TexCoord>],
    materials: &[Material],
    vfs: &MiniFs,
    path: P,
    texture_resolver: &dyn TextureResolver,
) -> Vec<radiance::rendering::Geometry> {
    let mut r_vertices = vec![];
    // let mut r_normals = vec![];
    for i in 0..vertices.len() {
        r_vertices.push(Vec3::new(vertices[i].x, vertices[i].y, vertices[i].z));
        // r_normals.push(Vec3::new(normals[i].x, normals[i].y, normals[i].z));
    }

    let r_texcoords: Vec<Vec<radiance::rendering::TexCoord>> = texcoord_sets
        .iter()
        .map(|t| {
            t.iter()
                .map(|t| radiance::rendering::TexCoord::new(t.u, t.v))
                .collect()
        })
        .collect();

    let mut material_to_indices = HashMap::new();

    struct MaterialGroupedIndices {
        material: MaterialDef,
        indices: Vec<u32>,
    }

    for t in triangles {
        let group = material_to_indices.entry(t.material).or_insert_with(|| {
            let material = &materials[t.material as usize];
            let md = if let Some(texture) = material.texture.as_ref() {
                let data = texture_resolver.resolve_texture(vfs, path.as_ref(), &texture.name);
                radiance::rendering::SimpleMaterialDef::create2(&texture.name, data, true)
            } else {
                radiance::rendering::SimpleMaterialDef::create2("missing", None, true)
            };

            MaterialGroupedIndices {
                material: md,
                indices: vec![],
            }
        });

        group.indices.push(t.index[0] as u32);
        group.indices.push(t.index[1] as u32);
        group.indices.push(t.index[2] as u32);
    }

    material_to_indices
        .into_values()
        .map(|v| {
            radiance::rendering::Geometry::new(
                &r_vertices,
                None,
                &r_texcoords,
                v.indices,
                v.material,
                1,
            )
        })
        .collect()
}
