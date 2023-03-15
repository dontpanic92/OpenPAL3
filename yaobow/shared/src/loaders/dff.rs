use std::{collections::HashMap, io::Read, path::Path, rc::Rc};

use crosscom::ComRc;
use fileformats::rwbs::{
    clump::Clump, frame::Frame, material::Material, read_dff, TexCoord, Triangle, Vec3f,
};
use mini_fs::{MiniFs, StoreExt};
use radiance::{
    comdef::{IEntity, IStaticMeshComponent},
    components::mesh::StaticMeshComponent,
    math::{Mat44, Vec3},
    rendering::{ComponentFactory, MaterialDef},
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

    let mut data = vec![];
    let _ = vfs.open(&path).unwrap().read_to_end(&mut data).unwrap();
    let chunks = read_dff(&data).unwrap();
    for chunk in chunks {
        load_clump(
            chunk,
            entity.clone(),
            component_factory,
            vfs,
            path.as_ref(),
            texture_resolver,
        );
    }
    entity
}

fn load_clump(
    chunk: Clump,
    parent: ComRc<IEntity>,
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: &Path,
    texture_resolver: &dyn TextureResolver,
) {
    let subs: Vec<ComRc<IEntity>> = chunk
        .frames
        .iter()
        .map(|f| {
            let entity = CoreEntity::create(format!("{}_sub", parent.name()), true);
            entity.transform().borrow_mut().set_matrix(create_matrix(f));
            entity
        })
        .collect();

    for i in 0..chunk.frames.len() {
        if chunk.frames[i].parent > 0 && chunk.frames[i].parent != i as i32 {
            subs[chunk.frames[i].parent as usize].attach(subs[i].clone());
        } else {
            parent.attach(subs[i].clone());
        }
    }

    for atomic in &chunk.atomics {
        let p = subs[atomic.frame as usize].clone();
        let entity = CoreEntity::create(format!("{}_sub", parent.name()), true);
        p.attach(entity.clone());

        let geometry = &chunk.geometries[atomic.geometry as usize];
        let r_geometry = create_geometry(geometry, vfs, &path, texture_resolver);

        let mesh_component =
            StaticMeshComponent::new(entity.clone(), r_geometry, component_factory.clone());
        entity.add_component(
            IStaticMeshComponent::uuid(),
            crosscom::ComRc::from_object(mesh_component),
        );
    }
}

fn create_matrix(frame: &Frame) -> Mat44 {
    let mut mat = Mat44::new_identity();
    mat.floats_mut()[0][0] = frame.right.x;
    mat.floats_mut()[1][0] = frame.right.y;
    mat.floats_mut()[2][0] = frame.right.z;
    mat.floats_mut()[0][1] = frame.up.x;
    mat.floats_mut()[1][1] = frame.up.y;
    mat.floats_mut()[2][1] = frame.up.z;
    mat.floats_mut()[0][2] = frame.at.x;
    mat.floats_mut()[1][2] = frame.at.y;
    mat.floats_mut()[2][2] = frame.at.z;
    mat.floats_mut()[0][3] = frame.pos.x;
    mat.floats_mut()[1][3] = frame.pos.y;
    mat.floats_mut()[2][3] = frame.pos.z;

    mat
}

fn create_geometry(
    geometry: &fileformats::rwbs::geometry::Geometry,
    vfs: &MiniFs,
    path: &Path,
    texture_resolver: &dyn TextureResolver,
) -> Vec<radiance::components::mesh::Geometry> {
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

pub(crate) fn create_geometry_internal(
    vertices: &[Vec3f],
    normals: Option<&Vec<Vec3f>>,
    triangles: &[Triangle],
    texcoord_sets: &[Vec<TexCoord>],
    materials: &[Material],
    vfs: &MiniFs,
    path: &Path,
    texture_resolver: &dyn TextureResolver,
) -> Vec<radiance::components::mesh::Geometry> {
    let mut r_vertices = vec![];
    // let mut r_normals = vec![];
    for i in 0..vertices.len() {
        r_vertices.push(Vec3::new(vertices[i].x, vertices[i].y, vertices[i].z));
        // r_normals.push(Vec3::new(normals[i].x, normals[i].y, normals[i].z));
    }

    let r_texcoords: Vec<Vec<radiance::components::mesh::TexCoord>> = texcoord_sets
        .iter()
        .map(|t| {
            t.iter()
                .map(|t| radiance::components::mesh::TexCoord::new(t.u, t.v))
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
            radiance::components::mesh::Geometry::new(
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
