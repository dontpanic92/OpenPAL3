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
    rendering::{
        AddressMode, AlphaKind, BlendMode, ComponentFactory, FilterMode, MaterialDef, SamplerDef,
    },
    scene::CoreEntity,
};

use super::TextureResolver;

pub struct DffLoaderConfig<'a> {
    pub texture_resolver: &'a dyn TextureResolver,
    pub keep_right_to_render_only: bool,
}

pub fn create_entity_from_dff_model<P: AsRef<Path>>(
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
    name: String,
    visible: bool,
    config: &DffLoaderConfig,
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
            config,
        );
    }

    entity
}

struct HAnimBone {
    bone_root: ComRc<IEntity>,
    bones: Vec<ComRc<IEntity>>,
    // Maps HAnim id (the value stored in `SkinPlugin.used_bones`) to the slot
    // in `bones` / the armature.
    hanim_id_to_slot: HashMap<u32, usize>,
    // For each slot, the bone's HAnim `index` field — i.e. the position in
    // `SkinPlugin.matrix` that holds its inverse-bind matrix.
    slot_to_hanim_index: Vec<u32>,
}

/// How a frame should be wired into the clump's entity hierarchy.
#[derive(Debug, PartialEq, Eq)]
enum FrameAttachment {
    /// Frame at index 0: aliased to the caller-provided clump root and
    /// therefore already in the scene graph (no attach needed).
    ImplicitRoot,
    /// Frame attaches directly to the caller-provided clump root. The
    /// `self_parent` flag distinguishes between an explicit "`parent == -1`"
    /// frame and a self-parent (`parent == own_index`) frame used by some
    /// PAL5 exporters to mean "attach to clump root".
    ClumpRoot { self_parent: bool },
    /// Frame attaches to another frame at the given index.
    Child(usize),
}

/// Classify how a frame should be wired given its `parent` field and its
/// own index in the clump's frame array.
fn frame_attachment(parent: i32, index: usize) -> FrameAttachment {
    if index == 0 {
        FrameAttachment::ImplicitRoot
    } else if parent < 0 {
        FrameAttachment::ClumpRoot { self_parent: false }
    } else if parent == index as i32 {
        FrameAttachment::ClumpRoot { self_parent: true }
    } else {
        FrameAttachment::Child(parent as usize)
    }
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
    config: &DffLoaderConfig,
) {
    let mut root_bone = None;
    let mut bone_id_map: HashMap<u32, ComRc<IEntity>> = HashMap::new();
    let entities: Vec<(ComRc<IEntity>, Option<ComRc<IEntity>>)> = chunk
        .frames
        .iter()
        .enumerate()
        .map(|(i, f)| {
            // RW's `frames[0]` is the implicit clump root with identity
            // transform. Aliasing it to the caller-provided `parent`
            // collapses the redundant transform level that would otherwise
            // appear under every clump.
            let entity = if i == 0 {
                debug_assert!(
                    is_identity_frame(f),
                    "DFF clump frames[0] expected to be identity transform",
                );
                parent.clone()
            } else {
                let e = CoreEntity::create(
                    f.name().unwrap_or(format!("{}_frame", parent.name())),
                    true,
                );
                let m = create_matrix(f);
                e.transform().as_ref().borrow_mut().set_matrix(m);
                e
            };
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

    let hanim_bone = if let Some((root_bone, hanim_bones_list)) = &root_bone {
        // Iterate HAnim bones in file order. The matrix-to-bone correspondence
        // is preserved via `slot_to_hanim_index` (used below to look up
        // `SkinPlugin.matrix[hanim_index]`), so slot order is independent of
        // the bones' `index` field. PAL5 exporters sometimes write bones in
        // author order rather than sorted by `index`; sorting here would
        // shuffle the slot-to-bone correspondence relative to the file.
        let mut bones = vec![];
        let mut hanim_id_to_slot: HashMap<u32, usize> = HashMap::new();
        let mut slot_to_hanim_index: Vec<u32> = vec![];
        for b in hanim_bones_list {
            let bone_entity = match bone_id_map.get(&b.id) {
                Some(e) => e.clone(),
                None => {
                    log::warn!(
                        "HAnim bone id {} referenced by hierarchy is missing an entity; skipping",
                        b.id
                    );
                    continue;
                }
            };
            let slot = bones.len();
            bones.push(bone_entity);
            hanim_id_to_slot.insert(b.id, slot);
            slot_to_hanim_index.push(b.index);
        }

        Some(HAnimBone {
            bone_root: root_bone.clone(),
            bones,
            hanim_id_to_slot,
            slot_to_hanim_index,
        })
    } else {
        None
    };

    for i in 0..chunk.frames.len() {
        match frame_attachment(chunk.frames[i].parent, i) {
            FrameAttachment::ImplicitRoot => {
                // entities[0].0 is aliased to `parent`; nothing to attach.
            }
            FrameAttachment::ClumpRoot { self_parent } => {
                if self_parent {
                    // Some PAL5 effect exporters write `parent = own_index`
                    // to mean "attach to clump root". Treating this as
                    // orphan (the historical behaviour) silently dropped
                    // the geometry; route it to the clump root instead.
                    log::trace!(
                        "DFF frame {} self-parents; attaching as clump root",
                        i
                    );
                }
                parent.attach(entities[i].0.clone());
            }
            FrameAttachment::Child(parent_id) => {
                entities[parent_id].0.attach(entities[i].0.clone());
                match (&entities[parent_id].1, &entities[i].1) {
                    (Some(parent_bone), Some(bone)) => {
                        parent_bone.attach(bone.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    let armature: Option<ComRc<IArmatureComponent>> = if let Some(hanim_bone) = &hanim_bone {
        let armature = ComRc::<IArmatureComponent>::from_object(ArmatureComponent::new(
            parent.clone(),
            hanim_bone.bone_root.clone(),
            hanim_bone.bones.clone(),
        ));
        parent.add_component(
            IArmatureComponent::uuid(),
            armature.clone().query_interface::<IComponent>().unwrap(),
        );
        Some(armature)
    } else {
        None
    };

    for atomic in &chunk.atomics {
        if config.keep_right_to_render_only && !atomic.contains_right_to_render() {
            continue;
        }

        let frame = &chunk.frames[atomic.frame as usize];
        if check_frame_name(frame) {
            continue;
        }

        if frame.is_hanim_bone() {
            // Atomics targeting a frame whose matrix_flags mark it as an
            // HAnim bone are unusual (typical for weapon-mount style
            // attachments). Log for diagnostics; do not skip — the
            // geometry is still legitimate.
            log::trace!(
                "DFF atomic targets HAnim bone frame {} (matrix_flags=0x{:08x})",
                atomic.frame,
                frame.matrix_flags(),
            );
        }

        let entity = entities[atomic.frame as usize].0.clone();

        let geometry = &chunk.geometries[atomic.geometry as usize];
        create_geometry(
            entity,
            component_factory,
            geometry,
            hanim_bone.as_ref(),
            armature.clone(),
            vfs,
            &path,
            config.texture_resolver,
        );
    }
}

fn check_frame_name(frame: &Frame) -> bool {
    for e in frame.extensions() {
        if let Extension::UserDataPlugin(plugin) = e {
            if let Some(prt) = plugin.data().get("prt") {
                if prt.len() > 0 {
                    if let Some(prt) = prt[0].get_string() {
                        if prt.starts_with('[') {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

fn create_geometry(
    entity: ComRc<IEntity>,
    component_factory: &Rc<dyn ComponentFactory>,
    geometry: &fileformats::rwbs::geometry::Geometry,
    hanim_bone: Option<&HAnimBone>,
    armature: Option<ComRc<IArmatureComponent>>,
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

        // Bind-pose assignment.
        //
        // `SkinPlugin.matrix` is indexed by each bone's HAnim `index` field,
        // not by the bone's slot in the armature. Walk the slots and look up
        // the matrix via `slot_to_hanim_index`.
        for (slot, &hanim_index) in hanim_bone.slot_to_hanim_index.iter().enumerate() {
            let hi = hanim_index as usize;
            if hi >= skin.matrix.len() {
                log::warn!(
                    "SkinPlugin: HAnim index {} out of range (matrix len {}); skipping bone slot {}",
                    hi,
                    skin.matrix.len(),
                    slot
                );
                continue;
            }
            let bone = hanim_bone.bones[slot].clone();
            let bond_pose = create_mat44_from_matrix44f(&skin.matrix[hi]);
            let bone_component = bone
                .get_component(IHAnimBoneComponent::uuid())
                .unwrap()
                .query_interface::<IHAnimBoneComponent>()
                .unwrap();

            bone_component.set_bond_pose(bond_pose);
        }

        // Per-vertex bone-index remap.
        //
        // `SkinPlugin.bone_indices[v][k]` indexes into `used_bones` (which is
        // a list of HAnim ids of the active bones). Build a single
        // `used_to_slot` table so the per-vertex loop is a flat lookup, then
        // collapse missing entries by zero-weighting the influence.
        const SLOT_MISSING: usize = usize::MAX;
        let used_to_slot: Vec<usize> = if !skin.used_bones.is_empty() {
            skin.used_bones
                .iter()
                .map(|&hid| {
                    hanim_bone
                        .hanim_id_to_slot
                        .get(&(hid as u32))
                        .copied()
                        .unwrap_or_else(|| {
                            log::warn!(
                                "SkinPlugin: used_bones entry HAnim id {} not present in HAnim hierarchy",
                                hid
                            );
                            SLOT_MISSING
                        })
                })
                .collect()
        } else {
            // Fallback for files that omit `used_bones`: indices address
            // `SkinPlugin.matrix` slots directly (i.e. the HAnim `index`
            // space). Translate that back to slot space via
            // `slot_to_hanim_index`.
            let max_hi = skin
                .matrix
                .len()
                .max(hanim_bone.slot_to_hanim_index.iter().map(|i| *i as usize + 1).max().unwrap_or(0));
            let mut hanim_index_to_slot = vec![SLOT_MISSING; max_hi];
            for (slot, &hi) in hanim_bone.slot_to_hanim_index.iter().enumerate() {
                let hi = hi as usize;
                if hi < hanim_index_to_slot.len() {
                    hanim_index_to_slot[hi] = slot;
                }
            }
            hanim_index_to_slot
        };

        let mut remapped_indices: Vec<[u8; 4]> = Vec::with_capacity(skin.bone_indices.len());
        let mut remapped_weights: Vec<[f32; 4]> = Vec::with_capacity(skin.weights.len());
        let mut warned_oob = false;
        let mut warned_missing = false;
        for (v, idxs) in skin.bone_indices.iter().enumerate() {
            let mut new_idx = [0u8; 4];
            let mut new_w = skin.weights[v];
            for k in 0..4 {
                let raw = idxs[k] as usize;
                let slot = if raw < used_to_slot.len() {
                    used_to_slot[raw]
                } else {
                    if !warned_oob {
                        log::warn!(
                            "SkinPlugin: per-vertex bone index {} >= used_to_slot len {}",
                            raw,
                            used_to_slot.len()
                        );
                        warned_oob = true;
                    }
                    SLOT_MISSING
                };

                if slot == SLOT_MISSING {
                    if !warned_missing {
                        log::warn!(
                            "SkinPlugin: dropping per-vertex bone influence that has no armature slot"
                        );
                        warned_missing = true;
                    }
                    new_idx[k] = 0;
                    new_w[k] = 0.0;
                } else {
                    // Slot must fit in u8: RW per-vertex skin indices are
                    // themselves u8s, so bones.len() <= 256.
                    debug_assert!(slot < 256, "armature has more than 256 bones");
                    new_idx[k] = slot as u8;
                }
            }
            remapped_indices.push(new_idx);
            remapped_weights.push(new_w);
        }

        Some(SkinnedMeshInfo {
            armature: armature.unwrap(),
            v_weights: remapped_weights,
            v_bone_indices: remapped_indices,
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

    let mut material_to_indices: Vec<(u16, MaterialGroupedIndices)> = Vec::new();

    struct MaterialGroupedIndices {
        material: MaterialDef,
        indices: Vec<u32>,
    }

    for t in triangles {
        let group_idx = match material_to_indices.iter().position(|(m, _)| *m == t.material) {
            Some(idx) => idx,
            None => {
                let material = &materials[t.material as usize];
                // RenderWare DFF carries blend info implicitly: the
                // material's RGBA `color` alpha byte signals translucency
                // and the texture's own alpha channel separates
                // alpha-cutout (binary) from alpha-blended (graded). See
                // `detect_blend` below.
                let md = if let Some(texture) = material.texture.as_ref() {
                    load_material_texture(texture, vfs, path.as_ref(), texture_resolver)
                } else {
                    log::debug!("no texture info for material {:?}", path);
                    radiance::rendering::SimpleMaterialDef::create2("missing", None)
                };

                let blend = detect_blend(material, &md);
                let md = md.with_blend(blend);

                material_to_indices.push((
                    t.material,
                    MaterialGroupedIndices {
                        material: md,
                        indices: vec![],
                    },
                ));
                material_to_indices.len() - 1
            }
        };

        let group = &mut material_to_indices[group_idx].1;
        group.indices.push(t.index[0] as u32);
        group.indices.push(t.index[1] as u32);
        group.indices.push(t.index[2] as u32);
    }

    let r_geometries = material_to_indices
        .into_iter()
        .map(|(_, v)| {
            // TODO: Optimize this
            radiance::components::mesh::Geometry::new(
                &r_vertices,
                None,
                &r_texcoords,
                v.indices,
                v.material,
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

/// Whether `frame` carries an identity rotation+translation. Used as a
/// debug-only sanity check when collapsing the implicit clump root.
fn is_identity_frame(frame: &Frame) -> bool {
    const EPS: f32 = 1e-5;
    let close = |a: f32, b: f32| (a - b).abs() <= EPS;
    close(frame.right.x, 1.0) && close(frame.right.y, 0.0) && close(frame.right.z, 0.0)
        && close(frame.up.x, 0.0) && close(frame.up.y, 1.0) && close(frame.up.z, 0.0)
        && close(frame.at.x, 0.0) && close(frame.at.y, 0.0) && close(frame.at.z, 1.0)
        && close(frame.pos.x, 0.0) && close(frame.pos.y, 0.0) && close(frame.pos.z, 0.0)
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

/// Map a RenderWare DFF material to a `BlendMode`. RW stores translucency
/// in two places: the material's RGBA `color` (alpha byte = "global"
/// material transparency) and the texture's own alpha channel.
///
/// - `mat_alpha < 255` → translucent material → `AlphaBlend`.
/// - texture is `AlphaKind::Blend` (truly graded alpha across many
///   pixels) → `AlphaBlend`.
/// - texture is `AlphaKind::Cutout` (mostly binary alpha) → `AlphaTest`.
///   The `AlphaTest` pipeline keeps depth-write on (so cutout meshes
///   stay in the cutout bucket and continue to occlude later transparent
///   draws against them) but uses premultiplied alpha-blend factors plus
///   a near-zero discard threshold, so bilinear-filtered edges still
///   look soft. Routing these to `AlphaBlend` would silently move
///   mostly-opaque atlases into the depth-write-off bucket and cause the
///   "see through the table" symptom: alpha-0 atlas texels stop
///   discarding and instead leave the destination unchanged.
/// - everything else → `Opaque`.
fn detect_blend(material: &Material, md: &MaterialDef) -> BlendMode {
    let mat_alpha = ((material.color >> 24) & 0xFF) as u8;
    let texture_alpha = md
        .textures()
        .first()
        .map(|t| t.alpha_kind())
        .unwrap_or(AlphaKind::Opaque);

    let blend = if mat_alpha < 255 || texture_alpha == AlphaKind::Blend {
        BlendMode::AlphaBlend
    } else if texture_alpha == AlphaKind::Cutout {
        BlendMode::AlphaTest
    } else {
        BlendMode::Opaque
    };

    blend
}

/// Decode a DFF material's texture (and optional alpha mask) into a
/// `MaterialDef`.
///
/// RenderWare materials reference up to two textures: `name` (the diffuse
/// RGB) and an optional `mask_name`. The mask is rendered as the alpha
/// channel of the diffuse: where the mask is bright the pixel is opaque,
/// where it's dark the pixel is transparent. PAL4 (and other RW games)
/// ships many cutout / glass textures this way — the diffuse is plain
/// RGB with no alpha channel of its own, so without composing the mask
/// we'd classify the surface as `Opaque` and write its (often black)
/// transparent regions into the depth/color buffer, occluding everything
/// behind it.
///
/// Decode a DFF material's texture (and optional alpha mask) into a
/// `MaterialDef`.
///
/// RenderWare materials reference up to two textures: `name` (the diffuse
/// RGB) and an optional `mask_name`. The mask is rendered as the alpha
/// channel of the diffuse: where the mask is bright the pixel is opaque,
/// where it's dark the pixel is transparent. PAL4 (and other RW games)
/// ships many cutout / glass textures this way — the diffuse is plain
/// RGB with no alpha channel of its own, so without composing the mask
/// we'd classify the surface as `Opaque` and write its (often black)
/// transparent regions into the depth/color buffer, occluding everything
/// behind it.
///
/// Returns a material whose single texture is the composited RGBA image.
/// The composite is cached under `"<main>|<mask>"` so repeated materials
/// sharing the same pair don't re-decode. The texture's `filter_mode`
/// and `address_mode_u/v` (parsed in
/// `fileformats/src/rwbs/material.rs`) are mapped to a cross-backend
/// `SamplerDef` via [`rw_sampler_def`] and forwarded to the material,
/// so CLAMP / MIRROR / BORDER addressing and NEAREST filtering reach
/// the GPU sampler instead of being silently dropped.
fn load_material_texture(
    texture: &fileformats::rwbs::material::Texture,
    vfs: &MiniFs,
    model_path: &Path,
    texture_resolver: &dyn TextureResolver,
) -> MaterialDef {
    let sampler = rw_sampler_def(texture);
    if texture.mask_name.is_empty() {
        return radiance::rendering::SimpleMaterialDef::create_with_sampler(
            &texture.name,
            |_name| {
                let data = texture_resolver.resolve_texture(vfs, model_path, &texture.name);
                if data.is_none() {
                    log::warn!(
                        "Failed to resolve texture {} for {:?}",
                        texture.name,
                        model_path
                    );
                }
                data.and_then(|data| Some(std::io::Cursor::new(data)))
            },
            sampler,
        );
    }

    let composite_name = format!("{}|{}", texture.name, texture.mask_name);
    let main_name = texture.name.clone();
    let mask_name = texture.mask_name.clone();
    let model_path_buf = model_path.to_path_buf();
    radiance::rendering::SimpleMaterialDef::create_with_image_and_sampler(
        &composite_name,
        {
            let main = decode_texture(vfs, &model_path_buf, &main_name, texture_resolver);
            let mask = decode_texture(vfs, &model_path_buf, &mask_name, texture_resolver);
            match (main, mask) {
                (Some(mut main), Some(mask)) => {
                    apply_mask_alpha(&mut main, &mask);
                    Some(main)
                }
                (Some(main), None) => {
                    log::warn!(
                        "Failed to resolve mask {} for {:?}; using diffuse alpha as-is",
                        mask_name,
                        model_path_buf
                    );
                    Some(main)
                }
                (None, _) => {
                    log::warn!(
                        "Failed to resolve texture {} for {:?}",
                        main_name,
                        model_path_buf
                    );
                    None
                }
            }
        },
        sampler,
    )
}

/// Build a cross-backend `SamplerDef` from the raw RenderWare
/// `Texture::filter_mode / address_mode_u / address_mode_v` fields
/// parsed in `fileformats/src/rwbs/material.rs`.
///
/// Filter mode mapping (RW values; see RW SDK `RwTextureFilterMode`):
///   0 = NAFILTERMODE              -> Linear (today's default)
///   1 = NEAREST                   -> Nearest
///   2 = LINEAR                    -> Linear
///   3 = MIPNEAREST                -> Nearest (mip levels not generated)
///   4 = MIPLINEAR                 -> Linear  (mip levels not generated)
///   5 = LINEARMIPNEAREST          -> Linear
///   6 = LINEARMIPLINEAR           -> Linear
///
/// Address mode mapping (RW `RwTextureAddressMode`):
///   0 = NATEXTUREADDRESS          -> Repeat (today's default)
///   1 = WRAP                      -> Repeat
///   2 = MIRROR                    -> Mirror
///   3 = CLAMP                     -> Clamp
///   4 = BORDER                    -> Border
///
/// Mip-map generation is intentionally skipped for now; once
/// `VulkanTexture` learns to build mip chains the `mipmap_mode` carried
/// on `SamplerDef` already takes effect (see
/// `radiance/.../vulkan/sampler.rs`).
fn rw_sampler_def(texture: &fileformats::rwbs::material::Texture) -> SamplerDef {
    let filter = match texture.filter_mode {
        1 | 3 => FilterMode::Nearest,
        2 | 4 | 5 | 6 => FilterMode::Linear,
        _ => FilterMode::Linear,
    };
    SamplerDef::with_address_uv(
        filter,
        rw_address_mode(texture.address_mode_u),
        rw_address_mode(texture.address_mode_v),
    )
}

fn rw_address_mode(mode: u32) -> AddressMode {
    match mode {
        1 => AddressMode::Repeat,
        2 => AddressMode::Mirror,
        3 => AddressMode::Clamp,
        4 => AddressMode::Border,
        _ => AddressMode::Repeat,
    }
}

fn decode_texture(
    vfs: &MiniFs,
    model_path: &Path,
    name: &str,
    texture_resolver: &dyn TextureResolver,
) -> Option<image::RgbaImage> {
    let data = texture_resolver.resolve_texture(vfs, model_path, name)?;
    image::load_from_memory(&data)
        .or_else(|_| image::load_from_memory_with_format(&data, image::ImageFormat::Tga))
        .ok()
        .map(|img| img.to_rgba8())
}

/// Composite a RenderWare mask texture onto `main`'s alpha channel.
///
/// RW mask textures are conventionally grayscale (bright = opaque, dark =
/// transparent). We sample the mask's luminance — averaging RGB lets us
/// handle masks stored as either grayscale or color without picking a
/// channel arbitrarily — and overwrite `main`'s alpha with it. If the
/// mask resolution differs from the main texture, sample with a nearest
/// lookup; mismatched resolutions are rare and a coarse sample is good
/// enough for cutout / blend classification.
fn apply_mask_alpha(main: &mut image::RgbaImage, mask: &image::RgbaImage) {
    let (mw, mh) = (main.width(), main.height());
    let (kw, kh) = (mask.width(), mask.height());
    if kw == 0 || kh == 0 {
        return;
    }

    let same_size = mw == kw && mh == kh;
    for y in 0..mh {
        for x in 0..mw {
            let mp = if same_size {
                *mask.get_pixel(x, y)
            } else {
                let mx = (x as u64 * kw as u64 / mw.max(1) as u64) as u32;
                let my = (y as u64 * kh as u64 / mh.max(1) as u64) as u32;
                *mask.get_pixel(mx.min(kw - 1), my.min(kh - 1))
            };
            // Luminance approximation; cheap and channel-agnostic.
            let luma = ((mp.0[0] as u16 + mp.0[1] as u16 + mp.0[2] as u16) / 3) as u8;
            main.get_pixel_mut(x, y).0[3] = luma;
        }
    }
}


#[cfg(test)]
mod hierarchy_tests {
    use super::*;
    use fileformats::rwbs::frame::FRAME_MATRIX_FLAG_HANIM_BONE;

    fn make_test_frame(parent: i32, matrix_flags: u32) -> Frame {
        Frame {
            right: Vec3f { x: 1.0, y: 0.0, z: 0.0 },
            up: Vec3f { x: 0.0, y: 1.0, z: 0.0 },
            at: Vec3f { x: 0.0, y: 0.0, z: 1.0 },
            pos: Vec3f { x: 0.0, y: 0.0, z: 0.0 },
            parent,
            matrix_flags,
            extensions: vec![],
        }
    }

    #[test]
    fn frame_attachment_index_zero_is_implicit_root_regardless_of_parent() {
        assert_eq!(frame_attachment(-1, 0), FrameAttachment::ImplicitRoot);
        assert_eq!(frame_attachment(0, 0), FrameAttachment::ImplicitRoot);
        assert_eq!(frame_attachment(42, 0), FrameAttachment::ImplicitRoot);
    }

    #[test]
    fn frame_attachment_negative_parent_is_clump_root_non_self() {
        assert_eq!(
            frame_attachment(-1, 3),
            FrameAttachment::ClumpRoot { self_parent: false }
        );
    }

    #[test]
    fn frame_attachment_self_parent_is_clump_root_self() {
        assert_eq!(
            frame_attachment(7, 7),
            FrameAttachment::ClumpRoot { self_parent: true }
        );
    }

    #[test]
    fn frame_attachment_other_parent_is_child() {
        assert_eq!(frame_attachment(2, 5), FrameAttachment::Child(2));
    }

    #[test]
    fn is_identity_frame_recognises_synthetic_identity() {
        let f = make_test_frame(-1, 0);
        assert!(is_identity_frame(&f));
    }

    #[test]
    fn is_identity_frame_rejects_translated_frame() {
        let mut f = make_test_frame(-1, 0);
        f.pos.x = 1.0;
        assert!(!is_identity_frame(&f));
    }

    #[test]
    fn frame_matrix_flags_bone_bit_round_trips() {
        let bone_frame = make_test_frame(0, FRAME_MATRIX_FLAG_HANIM_BONE);
        let plain_frame = make_test_frame(0, 0);
        assert!(bone_frame.is_hanim_bone());
        assert!(!plain_frame.is_hanim_bone());
    }
}
