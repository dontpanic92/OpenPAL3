use std::{collections::HashMap, io::Read, path::Path, rc::Rc};

use crosscom::ComRc;
use fileformats::rwbs::{
    clump::Clump, extension::Extension, frame::Frame, material::Material, read_dff, Matrix44f,
    TexCoord, Triangle, Vec3f,
};
use mini_fs::{MiniFs, StoreExt};
use radiance::{
    comdef::{
        IArmatureComponent, IComponent, IEntity, IHAnimBoneComponent, ISkinnedMeshComponent,
        IStaticMeshComponent,
    },
    components::mesh::{
        skinned_mesh::{ArmatureComponent, HAnimBoneComponent, SkinnedMeshComponent},
        StaticMeshComponent,
    },
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

struct HAnimBone {
    bone_root: ComRc<IEntity>,
    indexed_bones: HashMap<u32, ComRc<IEntity>>,
}

pub(crate) struct SkinnedMeshInfo {
    armature: ComRc<IArmatureComponent>,
    v_weights: Vec<[f32; 4]>,
    v_bone_indices: Vec<[u8; 4]>,
}

fn load_clump(
    chunk: Clump,
    parent: ComRc<IEntity>,
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: &Path,
    texture_resolver: &dyn TextureResolver,
) {
    let mut root_bone = None;
    let mut bone_id_map: HashMap<u32, ComRc<IEntity>> = HashMap::new();
    let entities: Vec<(ComRc<IEntity>, Option<ComRc<IEntity>>)> = chunk
        .frames
        .iter()
        .map(|f| {
            let entity =
                CoreEntity::create(f.name().unwrap_or(format!("{}_frame", parent.name())), true);
            let m = create_matrix(f);
            entity
                .transform()
                .as_ref()
                .borrow_mut()
                .set_matrix(m.clone());
            let bone = if let Some(hanim) = f.hanim_plugin() {
                let bone =
                    CoreEntity::create(format!("{}_bone", f.name().unwrap_or_default()), false);

                let bone_component = ComRc::<IComponent>::from_object(HAnimBoneComponent::new(
                    bone.clone(),
                    hanim.header.id,
                ));
                bone.add_component(IHAnimBoneComponent::uuid(), bone_component);
                bone_id_map.insert(hanim.header.id, bone.clone());

                if root_bone.is_none() {
                    root_bone = Some((bone.clone(), hanim.bones.clone()));
                }

                Some(bone)
            } else {
                None
            };

            (entity, bone)
        })
        .collect();

    let hanim_bone = if let Some((root_bone, index_map)) = &root_bone {
        let mut indexed_bones = HashMap::new();
        for b in index_map {
            indexed_bones.insert(b.index, bone_id_map.get(&b.id).unwrap().clone());
        }

        Some(HAnimBone {
            bone_root: root_bone.clone(),
            indexed_bones,
        })
    } else {
        None
    };

    for i in 0..chunk.frames.len() {
        if chunk.frames[i].parent < 0 {
            parent.attach(entities[i].0.clone());
        } else if chunk.frames[i].parent != i as i32 {
            let parent_id = chunk.frames[i].parent as usize;
            entities[parent_id].0.attach(entities[i].0.clone());
            match (&entities[parent_id].1, &entities[i].1) {
                (Some(parent_bone), Some(bone)) => {
                    parent_bone.attach(bone.clone());
                }
                _ => {}
            }
        } else {
            log::warn!("Ignored orphan frame");
        }
    }

    for atomic in &chunk.atomics {
        let entity = entities[atomic.frame as usize].0.clone();

        let geometry = &chunk.geometries[atomic.geometry as usize];
        create_geometry(
            entity,
            component_factory,
            geometry,
            hanim_bone.as_ref(),
            vfs,
            &path,
            texture_resolver,
        );
    }
}

fn create_geometry(
    entity: ComRc<IEntity>,
    component_factory: &Rc<dyn ComponentFactory>,
    geometry: &fileformats::rwbs::geometry::Geometry,
    hanim_bone: Option<&HAnimBone>,
    vfs: &MiniFs,
    path: &Path,
    texture_resolver: &dyn TextureResolver,
) {
    if geometry.morph_targets.len() == 0 {
        return;
    }

    if geometry.morph_targets[0].vertices.is_none() {
        return;
    }

    let vertices = geometry.morph_targets[0].vertices.as_ref().unwrap();
    let normals = geometry.morph_targets[0].normals.as_ref();
    let triangles = &geometry.triangles;
    let texcoord_sets = if geometry.texcoord_sets.len() >= 1 {
        vec![geometry.texcoord_sets[0].clone()]
    } else {
        vec![vertices.iter().map(|_| TexCoord { u: 0., v: 0. }).collect()]
    };
    let materials = &geometry.materials;

    let mut skin_plugin = None;
    for p in &geometry.extensions {
        if let Extension::SkinPlugin(plugin) = p {
            skin_plugin = Some(plugin);
            break;
        }
    }

    let skin_info = skin_plugin.and_then(|skin| {
        let hanim_bone = hanim_bone.unwrap();
        let mut bones = vec![];
        for i in 0..skin.matrix.len() {
            let bone = hanim_bone.indexed_bones.get(&(i as u32)).unwrap();
            let bond_pose = create_mat44_from_matrix44f(&skin.matrix[i]);
            let bone_component = bone
                .get_component(IHAnimBoneComponent::uuid())
                .unwrap()
                .query_interface::<IHAnimBoneComponent>()
                .unwrap();

            bone_component.set_bond_pose(bond_pose);
            bones.push(bone.clone());
        }

        let armature = entity
            .get_component(IArmatureComponent::uuid())
            .and_then(|c| c.query_interface::<IArmatureComponent>())
            .or_else(|| {
                let armature = ComRc::<IArmatureComponent>::from_object(ArmatureComponent::new(
                    entity.clone(),
                    hanim_bone.bone_root.clone(),
                    bones.clone(),
                ));
                entity.add_component(
                    IArmatureComponent::uuid(),
                    armature.clone().query_interface::<IComponent>().unwrap(),
                );
                Some(armature)
            })
            .unwrap();

        Some(SkinnedMeshInfo {
            armature,
            v_weights: skin.weights.clone(),
            v_bone_indices: skin.bone_indices.clone(),
        })
    });

    create_geometry_internal(
        entity,
        component_factory,
        vertices,
        normals,
        triangles,
        &texcoord_sets,
        materials,
        skin_info,
        vfs,
        path,
        texture_resolver,
    );
}

pub(crate) fn create_geometry_internal(
    entity: ComRc<IEntity>,
    component_factory: &Rc<dyn ComponentFactory>,
    vertices: &[Vec3f],
    _normals: Option<&Vec<Vec3f>>,
    triangles: &[Triangle],
    texcoord_sets: &[Vec<TexCoord>],
    materials: &[Material],
    skin_info: Option<SkinnedMeshInfo>,
    vfs: &MiniFs,
    path: &Path,
    texture_resolver: &dyn TextureResolver,
) {
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

    let r_geometries = material_to_indices
        .into_values()
        .map(|v| {
            // TODO: Optimize this
            radiance::components::mesh::Geometry::new(
                &r_vertices,
                None,
                &r_texcoords,
                v.indices,
                v.material,
                1,
            )
        })
        .collect();

    match skin_info {
        None => {
            let mesh_component =
                StaticMeshComponent::new(entity.clone(), r_geometries, component_factory.clone());
            entity.add_component(
                IStaticMeshComponent::uuid(),
                crosscom::ComRc::from_object(mesh_component),
            );
        }
        Some(skin_info) => {
            let bone_id: Vec<[usize; 4]> = skin_info
                .v_bone_indices
                .iter()
                .map(|id| {
                    [
                        id[0] as usize,
                        id[1] as usize,
                        id[2] as usize,
                        id[3] as usize,
                    ]
                })
                .collect();

            for r_geometry in r_geometries {
                let child = CoreEntity::create(format!("{}_geom", entity.name()), true);

                let mesh_component = SkinnedMeshComponent::new(
                    child.clone(),
                    component_factory.clone(),
                    r_geometry,
                    skin_info.armature.clone(),
                    bone_id.clone(),
                    skin_info.v_weights.clone(),
                );

                child.add_component(
                    ISkinnedMeshComponent::uuid(),
                    ComRc::from_object(mesh_component),
                );

                entity.attach(child);
            }
        }
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

fn create_mat44_from_matrix44f(m: &Matrix44f) -> Mat44 {
    let mut mat = Mat44::new_identity();
    mat.floats_mut()[0][0] = m.0[0];
    mat.floats_mut()[1][0] = m.0[1];
    mat.floats_mut()[2][0] = m.0[2];
    mat.floats_mut()[3][0] = m.0[3];
    mat.floats_mut()[0][1] = m.0[4];
    mat.floats_mut()[1][1] = m.0[5];
    mat.floats_mut()[2][1] = m.0[6];
    mat.floats_mut()[3][1] = m.0[7];
    mat.floats_mut()[0][2] = m.0[8];
    mat.floats_mut()[1][2] = m.0[9];
    mat.floats_mut()[2][2] = m.0[10];
    mat.floats_mut()[3][2] = m.0[11];
    mat.floats_mut()[0][3] = m.0[12];
    mat.floats_mut()[1][3] = m.0[13];
    mat.floats_mut()[2][3] = m.0[14];
    mat.floats_mut()[3][3] = 1.; //m.0[15];

    mat
}
