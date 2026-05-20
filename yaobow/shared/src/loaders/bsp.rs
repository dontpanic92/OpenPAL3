use std::{io::Read, path::Path, rc::Rc};

use anyhow::Context;
use crosscom::ComRc;
use fileformats::rwbs::{
    material::Material,
    read_bsp,
    sector::{AtomicSector, Sector},
};
use mini_fs::{MiniFs, StoreExt};
use radiance::{comdef::IEntity, rendering::ComponentFactory, scene::CoreEntity};

use super::dff::DffLoaderConfig;
use radiance::comdef::IEntityExt;

pub fn create_entity_from_bsp_model<P: AsRef<Path>>(
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
    name: String,
    config: &DffLoaderConfig,
) -> anyhow::Result<ComRc<IEntity>> {
    let entity = CoreEntity::create(name, true);
    load_bsp_model(entity.clone(), component_factory, vfs, path, config)?;

    Ok(entity)
}

fn load_bsp_model<P: AsRef<Path>>(
    entity: ComRc<IEntity>,
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
    config: &DffLoaderConfig,
) -> anyhow::Result<()> {
    let mut data = vec![];
    vfs.open(&path)
        .with_context(|| format!("opening BSP {}", path.as_ref().display()))?
        .read_to_end(&mut data)
        .with_context(|| format!("reading BSP {}", path.as_ref().display()))?;
    let chunks =
        read_bsp(&data).with_context(|| format!("parsing BSP {}", path.as_ref().display()))?;
    if !chunks.is_empty() {
        create_geometries(
            entity,
            component_factory,
            &chunks[0].sector,
            &chunks[0].materials,
            vfs,
            path,
            config,
        );
    }
    Ok(())
}

fn create_geometries<P: AsRef<Path>>(
    entity: ComRc<IEntity>,
    component_factory: &Rc<dyn ComponentFactory>,
    sector: &Sector,
    materials: &[Material],
    vfs: &MiniFs,
    path: P,
    config: &DffLoaderConfig,
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
                config,
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
                config,
            );
            create_geometries(
                entity.clone(),
                component_factory,
                &p.right_child,
                materials,
                vfs,
                path,
                config,
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
    config: &DffLoaderConfig,
) {
    let vertices = sector.vertices.as_ref();
    let _normals = sector.normals.as_ref();
    let triangles = &sector.triangles;

    let mut texcoord_sets = vec![];
    if let Some(t) = &sector.texcoords {
        texcoord_sets.push(t.clone());
    }

    if let Some(t) = &sector.texcoords2 {
        texcoord_sets.push(t.clone());
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
        vfs,
        path.as_ref(),
        config.texture_resolver,
        config.force_unique_materials,
        config.bsp_lightmap_tint,
    );
}
