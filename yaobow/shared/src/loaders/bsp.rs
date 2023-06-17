use std::{io::Read, path::Path, rc::Rc};

use crosscom::ComRc;
use fileformats::rwbs::{
    material::Material,
    read_bsp,
    sector::{AtomicSector, Sector},
};
use mini_fs::{MiniFs, StoreExt};
use radiance::{comdef::IEntity, rendering::ComponentFactory, scene::CoreEntity};

use super::TextureResolver;

pub fn create_entity_from_bsp_model<P: AsRef<Path>>(
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
    name: String,
    texture_resolver: &dyn TextureResolver,
) -> ComRc<IEntity> {
    let entity = CoreEntity::create(name, true);
    load_bsp_model(
        entity.clone(),
        component_factory,
        vfs,
        path,
        texture_resolver,
    );

    entity
}

fn load_bsp_model<P: AsRef<Path>>(
    entity: ComRc<IEntity>,
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
    texture_resolver: &dyn TextureResolver,
) {
    let mut data = vec![];
    let _ = vfs.open(&path).unwrap().read_to_end(&mut data).unwrap();
    let chunks = read_bsp(&data).unwrap();
    if !chunks.is_empty() {
        create_geometries(
            entity,
            component_factory,
            &chunks[0].sector,
            &chunks[0].materials,
            vfs,
            path,
            texture_resolver,
        );
    }
}

fn create_geometries<P: AsRef<Path>>(
    entity: ComRc<IEntity>,
    component_factory: &Rc<dyn ComponentFactory>,
    sector: &Sector,
    materials: &[Material],
    vfs: &MiniFs,
    path: P,
    texture_resolver: &dyn TextureResolver,
) {
    match sector {
        Sector::AtomicSector(a) => {
            create_geometry_from_atomic_sector(
                entity.clone(),
                component_factory,
                a,
                materials,
                vfs,
                path,
                texture_resolver,
            );
        }
        Sector::PlaneSector(p) => {
            create_geometries(
                entity.clone(),
                component_factory,
                &p.left_child,
                materials,
                vfs,
                path.as_ref(),
                texture_resolver,
            );
            create_geometries(
                entity.clone(),
                component_factory,
                &p.right_child,
                materials,
                vfs,
                path,
                texture_resolver,
            );
        }
    }
}

fn create_geometry_from_atomic_sector<P: AsRef<Path>>(
    entity: ComRc<IEntity>,
    component_factory: &Rc<dyn ComponentFactory>,
    sector: &AtomicSector,
    materials: &[Material],
    vfs: &MiniFs,
    path: P,
    texture_resolver: &dyn TextureResolver,
) {
    let vertices = sector.vertices.as_ref();
    let _normals = sector.normals.as_ref();
    let triangles = &sector.triangles;

    let mut texcoord_sets = vec![];
    if let Some(t) = &sector.texcoords {
        texcoord_sets.push(t.clone());
    }

    if let Some(_t) = &sector.texcoords2 {
        // texcoord_sets.push(t.clone());
    }

    let child = CoreEntity::create(format!("{}_sub", entity.name()), true);
    entity.attach(child.clone());

    super::dff::create_geometry_internal(
        child,
        component_factory,
        vertices,
        None,
        &triangles,
        &texcoord_sets,
        materials,
        None,
        None,
        vfs,
        path.as_ref(),
        texture_resolver,
    );
}
