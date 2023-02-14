use std::{io::Read, path::Path, rc::Rc};

use crosscom::ComRc;
use fileformats::rwbs::{
    material::Material,
    read_bsp,
    sector::{AtomicSector, Sector},
};
use mini_fs::{MiniFs, StoreExt};
use radiance::{
    comdef::{IEntity, IStaticMeshComponent},
    rendering::{ComponentFactory, Geometry, StaticMeshComponent},
    scene::CoreEntity,
};

use super::TextureResolver;

pub fn create_entity_from_bsp_model<P: AsRef<Path>>(
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
    name: String,
    texture_resolver: &dyn TextureResolver,
) -> ComRc<IEntity> {
    let entity = CoreEntity::create(name, true);
    let geometries = load_bsp_model(vfs, path, texture_resolver);
    let mesh_component =
        StaticMeshComponent::new(entity.clone(), geometries, component_factory.clone());
    entity.add_component(
        IStaticMeshComponent::uuid(),
        crosscom::ComRc::from_object(mesh_component),
    );
    entity
}

fn load_bsp_model<P: AsRef<Path>>(
    vfs: &MiniFs,
    path: P,
    texture_resolver: &dyn TextureResolver,
) -> Vec<radiance::rendering::Geometry> {
    let mut data = vec![];
    let _ = vfs.open(&path).unwrap().read_to_end(&mut data).unwrap();
    let chunks = read_bsp(&data).unwrap();
    if chunks.is_empty() {
        vec![]
    } else {
        let r_geometries = create_geometries(
            &chunks[0].sector,
            &chunks[0].materials,
            vec![],
            vfs,
            path,
            texture_resolver,
        );
        r_geometries
    }
}

fn create_geometries<P: AsRef<Path>>(
    sector: &Sector,
    materials: &[Material],
    mut geometries: Vec<Geometry>,
    vfs: &MiniFs,
    path: P,
    texture_resolver: &dyn TextureResolver,
) -> Vec<Geometry> {
    match sector {
        Sector::AtomicSector(a) => {
            geometries.append(&mut create_geometry_from_atomic_sector(
                a,
                materials,
                vfs,
                path,
                texture_resolver,
            ));
            geometries
        }
        Sector::PlaneSector(p) => {
            let geometries = create_geometries(
                &p.left_child,
                materials,
                geometries,
                vfs,
                path.as_ref(),
                texture_resolver,
            );
            create_geometries(
                &p.right_child,
                materials,
                geometries,
                vfs,
                path,
                texture_resolver,
            )
        }
    }
}

fn create_geometry_from_atomic_sector<P: AsRef<Path>>(
    sector: &AtomicSector,
    materials: &[Material],
    vfs: &MiniFs,
    path: P,
    texture_resolver: &dyn TextureResolver,
) -> Vec<Geometry> {
    let vertices = sector.vertices.as_ref();
    let normals = sector.normals.as_ref();
    let triangles = &sector.triangles;

    let mut texcoord_sets = vec![];
    if let Some(t) = &sector.texcoords {
        texcoord_sets.push(t.clone());
    }

    if let Some(t) = &sector.texcoords2 {
        // texcoord_sets.push(t.clone());
    }

    super::dff::create_geometry_internal(
        vertices,
        None,
        &triangles,
        &texcoord_sets,
        materials,
        vfs,
        path,
        texture_resolver,
    )
}
