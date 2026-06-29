use std::{collections::HashMap, io::Read, path::Path, rc::Rc};

use anyhow::Context;
use crosscom::ComRc;
use fileformats::rwbs::{
    Matrix44f, TexCoord, Triangle, Vec3f, clump::Clump, extension::Extension, frame::Frame,
    material::Material, read_dff,
};
use mini_fs::{MiniFs, StoreExt};
use radiance::{
    comdef::{
        IArmatureComponent, IBillboardComponent, IComponent, IEntity, IHAnimBoneComponent,
        ISkinnedMeshComponent, IStaticMeshComponent,
    },
    components::billboard::BillboardComponent,
    components::mesh::{
        StaticMeshComponent,
        skinned_mesh::{ArmatureComponent, HAnimBoneComponent, SkinnedMeshComponent},
    },
    math::{Mat44, Vec3},
    rendering::{
        AddressMode, AlphaKind, BlendMode, ComponentFactory, CullMode, FilterMode, MaterialDef,
        SamplerDef,
    },
    scene::CoreEntity,
};

use super::FoliageResolver;
use super::TextureResolver;
use radiance::comdef::{IEntityExt, IHAnimBoneComponentExt};

pub struct DffLoaderConfig<'a> {
    pub texture_resolver: &'a dyn TextureResolver,
    pub keep_right_to_render_only: bool,
    /// When `true`, every `MaterialDef` built from this DFF is stamped
    /// with [`MaterialDef::make_unique`], opting it out of the
    /// renderer's identity-keyed material cache. Required for DFFs
    /// whose materials get mutated at runtime (PAL4 `_water.dff`s) so
    /// the per-frame UV transform doesn't leak onto unrelated geometry
    /// (grass / leaves / hair) that happens to share the same texture.
    /// Default `false` so non-animated DFFs keep benefiting from the
    /// cache.
    pub force_unique_materials: bool,
    /// When `true`, the translation column of every clump-root frame
    /// (`parent == -1`) is forced to zero before being applied to the
    /// frame's entity. PAL4 game-object DFFs bake a small "world-rest"
    /// pivot into the root frame; the original engine binds geometry
    /// directly at the caller-supplied world position and ignores the
    /// pivot, so without this flag meshes end up at
    /// `entry.position + R·frames[0].pos` — a small, mostly-vertical
    /// gap. Actor/world/water DFFs do not have this pivot and should
    /// leave the flag `false`. Frame *rotation* is untouched so
    /// authored axis conventions are preserved.
    pub ignore_root_frame_translation: bool,
    /// Per-scene lightmap modulation, packed as
    /// `[tint.r, tint.g, tint.b, intensity]` (from
    /// `<block>_ltMap.cfg`). Used only by the BSP loader path: when
    /// `Some`, materials whose `texture.name` contains the substring
    /// `"LightingMap"` are upgraded to the `TexturedLightmap` shader
    /// (`LightMapMaterialDef` with `[lightmap, diffuse]` textures and
    /// the BSP's second UV set), and the value is stamped into
    /// `MaterialParams.tint` so `lightmap_texture.frag`'s
    /// `rgb * mat.tint.rgb * mat.tint.a` modulates the bake by the
    /// per-scene mood (e.g. `Q01` day vs `Q01Y` night). The diffuse
    /// texture is the same path with `"LightingMap"` substituted for
    /// `"DiffuseMap"`, matching the on-disk pairing convention. Default
    /// `None` so DFF callers and non-PAL4 BSPs are unaffected.
    pub bsp_lightmap_tint: Option<[f32; 4]>,

    /// When `true`, plain (non-lightmap) textured DFF materials are built
    /// with the dynamically-lit shader ([`ShaderProgram::TexturedDynamicLit`])
    /// instead of the unlit one, and the mesh's vertex normals are forwarded
    /// to the GPU. This makes scene objects (buildings, props) respond to the
    /// scene's directional sun + ambient (`SceneLighting`) the same way PAL3
    /// actors and PAL5 terrain do. Requires the DFF geometry to carry
    /// normals; materials without normals fall back to the unlit shader.
    /// Default `false` so PAL3/PAL4/SWD keep their existing unlit/baked look.
    pub dynamic_lighting: bool,

    /// When `true`, every `MaterialDef` built from this DFF is stamped with
    /// [`MaterialParams::fog_exempt`], so the world-geometry shaders skip the
    /// scene's linear distance fog for this model. Used for the PAL5 skybox,
    /// whose camera-locked dome encloses the whole scene and would otherwise
    /// read as a constant far-distance fog wash. Default `false`.
    pub fog_exempt: bool,

    /// PAL5 leaf/sprite-card resolver. Many PAL5 tree-leaf quads are tagged
    /// `[W]/[w]{t<id>}` and ship with **no texture and no UV set** — the engine
    /// supplies both at load time from `Config/uvlist.tb`, keyed by `{t<id>}`.
    /// When set, such quads are textured + UV-mapped from the resolved
    /// [`FoliageCard`] instead of being dropped. Only PAL5 wires this; other
    /// games leave it `None` (their DFFs carry no `prt` tag, so the path is a
    /// no-op for them anyway). See `generated/pal5_leaf_re.md`.
    pub foliage_resolver: Option<&'a dyn FoliageResolver>,
}

impl<'a> DffLoaderConfig<'a> {
    /// Constructor with default (`force_unique_materials = false`).
    pub fn new(texture_resolver: &'a dyn TextureResolver) -> Self {
        Self {
            texture_resolver,
            keep_right_to_render_only: false,
            force_unique_materials: false,
            ignore_root_frame_translation: false,
            bsp_lightmap_tint: None,
            dynamic_lighting: false,
            fog_exempt: false,
            foliage_resolver: None,
        }
    }
}

pub fn create_entity_from_dff_model<P: AsRef<Path>>(
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
    name: String,
    visible: bool,
    config: &DffLoaderConfig,
) -> anyhow::Result<ComRc<IEntity>> {
    let entity = CoreEntity::create(name, visible);

    let mut data = vec![];
    vfs.open(&path)
        .with_context(|| format!("opening DFF {}", path.as_ref().display()))?
        .read_to_end(&mut data)
        .with_context(|| format!("reading DFF {}", path.as_ref().display()))?;
    let chunks =
        read_dff(&data).with_context(|| format!("parsing DFF {}", path.as_ref().display()))?;
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

    Ok(entity)
}

struct HAnimBone {
    bone_root: ComRc<IEntity>,
    bones: Vec<ComRc<IEntity>>,
    // For each slot, the bone's HAnim `index` field — i.e. the position in
    // `SkinPlugin.matrix` that holds its inverse-bind matrix, and also the
    // value RW writes into per-vertex `SkinPlugin.bone_indices`.
    slot_to_hanim_index: Vec<u32>,
}

/// How a frame should be wired into the clump's entity hierarchy.
#[derive(Debug, PartialEq, Eq)]
enum FrameAttachment {
    /// Frame attaches directly to the caller-provided clump root
    /// (`parent == -1`).
    ClumpRoot,
    /// Frame whose `parent` field equals its own index. Some exporters use
    /// this as a sentinel for "no parent"; historically these frames were
    /// ignored as orphans. Preserved as a distinct case so callers can log
    /// without changing scene-graph behaviour.
    SelfParented,
    /// Frame attaches to another frame at the given index.
    Child(usize),
}

/// Classify how a frame should be wired given its `parent` field and its
/// own index in the clump's frame array.
fn frame_attachment(parent: i32, index: usize) -> FrameAttachment {
    if parent < 0 {
        FrameAttachment::ClumpRoot
    } else if parent == index as i32 {
        FrameAttachment::SelfParented
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
            let entity =
                CoreEntity::create(f.name().unwrap_or(format!("{}_frame", parent.name())), true);
            let mut m = create_matrix(f);
            // For clump-root frames, optionally strip the translation
            // column. PAL4 game-object DFFs bake a small world-rest
            // pivot into the root frame that the original engine
            // ignores (geometry binds directly at the caller-supplied
            // world position). See `DffLoaderConfig::ignore_root_frame_translation`.
            if config.ignore_root_frame_translation
                && frame_attachment(f.parent, i) == FrameAttachment::ClumpRoot
            {
                m.floats_mut()[0][3] = 0.;
                m.floats_mut()[1][3] = 0.;
                m.floats_mut()[2][3] = 0.;
            }
            entity.transform().as_ref().borrow_mut().set_matrix(m);
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
        let mut bones = vec![];
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
            bones.push(bone_entity);
            slot_to_hanim_index.push(b.index);
        }

        Some(HAnimBone {
            bone_root: root_bone.clone(),
            bones,
            slot_to_hanim_index,
        })
    } else {
        None
    };

    for i in 0..chunk.frames.len() {
        match frame_attachment(chunk.frames[i].parent, i) {
            FrameAttachment::ClumpRoot => {
                parent.attach(entities[i].0.clone());
            }
            FrameAttachment::SelfParented => {
                // Historical behaviour: treat frames whose parent equals
                // their own index as orphans and drop them. Re-routing them
                // to the clump root changes geometry placement, so we keep
                // them dropped and only warn.
                log::warn!("Ignored orphan frame");
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

    // A clump that carries an HAnim skeleton but *no* skinned geometry is
    // a rigid, frame-hierarchy-animated prop (PAL4 doors, levers, tip
    // markers, …). There is nothing to deform: the visible mesh hangs off
    // a bone frame and the `.anm` drives that frame's local transform.
    // We therefore animate the *frame* entities themselves — they carry
    // the static mesh and live in the scene graph, so the engine ticks
    // their `HAnimBoneComponent`s and propagates world placement — and
    // install a `frame_driven` armature that only owns the shared
    // timeline (loop reset / hold). Skinned actors keep the original
    // detached-skeleton path.
    let has_skinned_geometry = chunk.geometries.iter().any(|g| {
        g.extensions
            .iter()
            .any(|e| matches!(e, fileformats::rwbs::extension::Extension::SkinPlugin(_)))
    });

    let armature: Option<ComRc<IArmatureComponent>> = match (&hanim_bone, has_skinned_geometry) {
        (Some(hanim_bone), true) => {
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
        }
        (Some(_), false) => {
            // Rigid prop: attach a bone component to each animated frame
            // entity, in the same HAnim-hierarchy order the `.anm` tracks
            // are stored in, then build a frame-driven armature over them.
            let mut id_to_frame: HashMap<u32, ComRc<IEntity>> = HashMap::new();
            for (i, f) in chunk.frames.iter().enumerate() {
                if let Some(h) = f.hanim_plugin() {
                    id_to_frame.insert(h.header.id, entities[i].0.clone());
                }
            }

            let hanim_bones_list = &root_bone.as_ref().unwrap().1;
            let mut frame_bones = vec![];
            for b in hanim_bones_list {
                if let Some(frame_entity) = id_to_frame.get(&b.id) {
                    let bone_component = ComRc::<IComponent>::from_object(HAnimBoneComponent::new(
                        frame_entity.clone(),
                        b.id,
                    ));
                    frame_entity.add_component(IHAnimBoneComponent::uuid(), bone_component);
                    frame_bones.push(frame_entity.clone());
                }
            }

            if frame_bones.is_empty() {
                None
            } else {
                let armature = ComRc::<IArmatureComponent>::from_object(
                    ArmatureComponent::new_frame_driven(parent.clone(), frame_bones),
                );
                parent.add_component(
                    IArmatureComponent::uuid(),
                    armature.clone().query_interface::<IComponent>().unwrap(),
                );
                Some(armature)
            }
        }
        (None, _) => None,
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

        // PAL5 foliage cards are single quads (render two-sided) and
        // wind-leaf clusters are flat footprint quads that must face the
        // camera each frame (billboard). Both are detected purely from
        // the frame's `prt` tag, which only PAL5 ships — so non-PAL5
        // DFFs are unaffected.
        let two_sided = is_foliage_frame(frame);
        let billboard = is_billboard_frame(frame);

        let geometry = &chunk.geometries[atomic.geometry as usize];

        // Texture-less PAL5 wind-billboard leaves. Some trees (e.g.
        // `zw_shulin`, `zw_dongzhu`) ship `[w]/[W]` leaf quads whose
        // material carries no texture (RW `textured=0`) and no UV set —
        // PAL5's engine supplies the leaf texture+UVs at load time from the
        // sprite/foliage subsystem, keyed by the frame's `{t<id>}` tag into
        // `Config/uvlist.tb` (see `generated/pal5_leaf_re.md`). When a
        // `foliage_resolver` is wired (PAL5), resolve the card and render it
        // with a synthesized texture + UVs; otherwise drop the quad rather
        // than render the magenta "missing" placeholder. Textured leaves are
        // unaffected.
        if billboard && !geometry_has_texture(geometry) {
            match config
                .foliage_resolver
                .zip(prt_texture_id(frame))
                .and_then(|(r, id)| r.resolve_card(id))
            {
                Some(card) => {
                    create_foliage_geometry(
                        entity.clone(),
                        component_factory,
                        geometry,
                        &card,
                        vfs,
                        &path,
                        config.texture_resolver,
                        config.force_unique_materials,
                        config.dynamic_lighting,
                        config.fog_exempt,
                    );
                    let component =
                        BillboardComponent::create(entity.clone(), billboard_scale_pct(frame));
                    entity.add_component(
                        IBillboardComponent::uuid(),
                        component.query_interface::<IComponent>().unwrap(),
                    );
                }
                None => {}
            }
            continue;
        }

        create_geometry(
            entity.clone(),
            component_factory,
            geometry,
            hanim_bone.as_ref(),
            armature.clone(),
            vfs,
            &path,
            config.texture_resolver,
            config.force_unique_materials,
            config.bsp_lightmap_tint,
            two_sided,
            config.dynamic_lighting,
            config.fog_exempt,
        );

        if billboard {
            let component = BillboardComponent::create(entity.clone(), billboard_scale_pct(frame));
            entity.add_component(
                IBillboardComponent::uuid(),
                component.query_interface::<IComponent>().unwrap(),
            );
        }
    }
}

/// Whether any of a geometry's materials reference a (named) texture.
/// Used to drop texture-less PAL5 billboard leaves that would otherwise
/// render as the magenta "missing" placeholder.
fn geometry_has_texture(geometry: &fileformats::rwbs::geometry::Geometry) -> bool {
    geometry.materials.iter().any(|m| {
        m.texture
            .as_ref()
            .map(|t| !t.name.is_empty())
            .unwrap_or(false)
    })
}

/// Decide whether an atomic's frame is a **non-renderable helper** that
/// must be skipped at load time.
///
/// PAL5 stamps a `prt` UserData string on most frames. A leading
/// bracket token classifies the frame; the meanings observed across
/// PAL5 assets are:
///
/// | tag(s)                         | meaning                | render? |
/// |--------------------------------|------------------------|---------|
/// | `[W]` / `[w]`                  | wind-animated foliage  | **yes** |
/// | `[(]` / `[$(]` / `[($]`        | billboard plant / bush | **yes** |
/// | `[^]` / `[^~]`                 | collision / clip helper| no      |
/// | `[S]` / `[ST]`                 | shadow helper          | no      |
///
/// The original loader skipped *every* bracketed `prt` frame, which
/// silently dropped all wind foliage and bushes (e.g. `zw_gushu.dff`
/// rendered 1 of its 124 atomics — the bare trunk — because the 123
/// leaf clusters are tagged `[W]{t..}{s..}gushuye..`). We now skip only
/// the helper categories (`^`, `~`, `S`) and render everything else.
///
/// Non-PAL5 DFFs (PAL3/PAL4/SWD) carry no `prt` UserData, so this is a
/// no-op for them.
/// Read a frame's `prt` UserData string (PAL5 stamps these; other games
/// don't carry the plugin).
fn frame_prt(frame: &Frame) -> Option<String> {
    for e in frame.extensions() {
        if let Extension::UserDataPlugin(plugin) = e {
            if let Some(prt) = plugin.data().get("prt") {
                if let Some(s) = prt.get(0).and_then(|x| x.get_string()) {
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            }
        }
    }
    None
}

fn check_frame_name(frame: &Frame) -> bool {
    if let Some(prt) = frame_prt(frame) {
        if let Some(rest) = prt.strip_prefix('[') {
            // First character inside the bracket selects the frame
            // category. Helper geometry (collision `^`, clip `~`, shadow
            // `S`) is skipped; foliage / billboard tags render.
            return matches!(rest.chars().next(), Some('^') | Some('~') | Some('S'));
        }
    }

    false
}

/// Whether a frame is renderable PAL5 foliage: a bracketed `prt` tag
/// that survived [`check_frame_name`] (wind foliage `[W]`/`[w]`,
/// billboard plants `[(]`/`[$(]`/`[($]`, …). These are single-quad
/// cards that must render two-sided so the back-facing half is not
/// culled. Non-PAL5 DFFs have no `prt`, so this is always `false`.
fn is_foliage_frame(frame: &Frame) -> bool {
    frame_prt(frame)
        .map(|p| p.starts_with('[') && !check_frame_name(frame))
        .unwrap_or(false)
}

/// Whether a frame is a PAL5 wind-billboard leaf cluster (`[W]`/`[w]`):
/// a flat footprint quad that must be oriented toward the camera each
/// frame by a [`BillboardComponent`].
fn is_billboard_frame(frame: &Frame) -> bool {
    frame_prt(frame)
        .map(|p| p.starts_with("[W]") || p.starts_with("[w]"))
        .unwrap_or(false)
}

/// Parse the per-leaf scale percentage from a PAL5 wind `prt` tag of the
/// form `[W]{t<id>}{s<pct>}<name>` (e.g. `[W]{t6091}{s90}gushuye03` →
/// `90`). Returns `100` (no scaling) when the `{s..}` token is absent.
fn billboard_scale_pct(frame: &Frame) -> f32 {
    let Some(prt) = frame_prt(frame) else {
        return 100.0;
    };
    if let Some(rest) = prt.split("{s").nth(1) {
        if let Some(num) = rest.split('}').next() {
            if let Ok(v) = num.trim().parse::<f32>() {
                if v > 0.0 {
                    return v;
                }
            }
        }
    }
    100.0
}

/// Parse the leaf-card texture id from a PAL5 wind `prt` tag of the form
/// `[W]{t<id>}{s<pct>}<name>` (e.g. `[w]{t6140}{s60}leaf02` → `6140`). This is
/// the `Config/uvlist.tb` key (see [`FoliageResolver`]). Returns `None` when
/// the `{t..}` token is absent or unparseable.
fn prt_texture_id(frame: &Frame) -> Option<u32> {
    parse_prt_texture_id(&frame_prt(frame)?)
}

fn parse_prt_texture_id(prt: &str) -> Option<u32> {
    let rest = prt.split("{t").nth(1)?;
    let num = rest.split('}').next()?;
    num.trim().parse::<u32>().ok()
}

/// Footprint-to-card enlargement for untextured PAL5 leaf sprites, overridable
/// via the `PAL5_LEAF_FOOTPRINT_GAIN` env var.
///
/// PAL5's untextured leaf quads are a **uniform 10.6×21.2 "footprint" marker**
/// (identical across `zw_shulin_07/002A/008A/…`), not the real leaf size — the
/// engine draws the uvlist sprite larger than the marker. Trees whose leaf
/// cards carry an embedded full-size texture instead author them at canopy
/// scale (`zw_gushu` ≈ 34, `zw_rongshu` ≈ 20–41). The default `4.0` lifts the
/// 10.6 marker into that textured-card range so footprint trees fill out to
/// match; validated visually on `--pal5 kuangfengzhai` (the ginkgo/`yinxingqiu`
/// forest trees render as full canopies rather than sparse poles).
fn foliage_footprint_gain() -> f32 {
    use std::sync::OnceLock;
    static GAIN: OnceLock<f32> = OnceLock::new();
    *GAIN.get_or_init(|| {
        std::env::var("PAL5_LEAF_FOOTPRINT_GAIN")
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|v: &f32| *v > 0.0)
            .unwrap_or(4.0)
    })
}

/// Render a texture-less PAL5 leaf quad using a resolved [`FoliageCard`]: stamp
/// the card's atlas texture onto the quad's material(s) and synthesize
/// per-vertex UVs from the card's UV rectangle, then build the mesh.
///
/// The leaf quad lies flat in its frame's local XZ plane (the engine erects it
/// toward the camera via [`BillboardComponent`]). We map `x → u` and `z → v`
/// across the quad's bounding box, with `+z` at the top of the atlas, so the
/// whole `(u0,u1,v0,v1)` sub-rect covers the card. Leaf cards always render
/// two-sided so the back-facing half is not culled.
fn create_foliage_geometry(
    entity: ComRc<IEntity>,
    component_factory: &Rc<dyn ComponentFactory>,
    geometry: &fileformats::rwbs::geometry::Geometry,
    card: &super::FoliageCard,
    vfs: &MiniFs,
    path: &Path,
    texture_resolver: &dyn TextureResolver,
    force_unique_materials: bool,
    dynamic_lighting: bool,
    fog_exempt: bool,
) {
    let Some(vertices) = geometry
        .morph_targets
        .get(0)
        .and_then(|m| m.vertices.as_ref())
    else {
        return;
    };
    let normals = geometry.morph_targets[0].normals.as_ref();

    let atlas_texture = fileformats::rwbs::material::Texture {
        // Wrap addressing + linear filtering for the atlas sample.
        filter_mode: 2,
        address_mode_u: 1,
        address_mode_v: 1,
        name: card.atlas.clone(),
        mask_name: String::new(),
    };
    // Keep each original material's color (per-leaf tint) but point it at the
    // resolved leaf atlas. The quad ships at least one material; guard the
    // empty case so an odd export still renders.
    let mut materials: Vec<Material> = geometry
        .materials
        .iter()
        .map(|m| {
            let mut m = m.clone();
            m.texture = Some(atlas_texture.clone());
            m
        })
        .collect();
    if materials.is_empty() {
        materials.push(Material {
            color: 0xffff_ffff,
            texture: Some(atlas_texture),
            ..Default::default()
        });
    }

    let [u0, u1, v0, v1] = card.uv;
    let (mut min_x, mut max_x) = (f32::INFINITY, f32::NEG_INFINITY);
    let (mut min_z, mut max_z) = (f32::INFINITY, f32::NEG_INFINITY);
    for v in vertices {
        min_x = min_x.min(v.x);
        max_x = max_x.max(v.x);
        min_z = min_z.min(v.z);
        max_z = max_z.max(v.z);
    }
    let span_x = (max_x - min_x).max(1e-6);
    let span_z = (max_z - min_z).max(1e-6);
    let texcoords: Vec<Vec<TexCoord>> = vec![
        vertices
            .iter()
            .map(|v| {
                let fx = (v.x - min_x) / span_x;
                let fz = (max_z - v.z) / span_z; // +z → top of atlas
                TexCoord {
                    u: u0 + fx * (u1 - u0),
                    v: v0 + fz * (v1 - v0),
                }
            })
            .collect(),
    ];

    // PAL5's untextured leaf quads are small "footprint" markers (a uniform
    // 10.6×21.2 across `zw_shulin_*`) — much smaller than trees whose leaf
    // cards carry an embedded full-size texture (`zw_gushu` ≈ 34×34). Rendered
    // at their footprint size they look sparse, so the original engine draws
    // the uvlist sprite larger than the marker quad. Scale the footprint about
    // its own centroid by `foliage_footprint_gain()` (see there for the value).
    let footprint_gain = foliage_footprint_gain();
    let scaled;
    let vertices: &[Vec3f] = if (footprint_gain - 1.0).abs() < 1e-3 {
        vertices
    } else {
        let cx = 0.5 * (min_x + max_x);
        let cz = 0.5 * (min_z + max_z);
        let cy = 0.5
            * (vertices.iter().map(|v| v.y).fold(f32::INFINITY, f32::min)
                + vertices
                    .iter()
                    .map(|v| v.y)
                    .fold(f32::NEG_INFINITY, f32::max));
        scaled = vertices
            .iter()
            .map(|v| Vec3f {
                x: cx + (v.x - cx) * footprint_gain,
                y: cy + (v.y - cy) * footprint_gain,
                z: cz + (v.z - cz) * footprint_gain,
            })
            .collect::<Vec<_>>();
        &scaled
    };

    create_geometry_internal(
        entity,
        component_factory,
        vertices,
        normals,
        &geometry.triangles,
        &texcoords,
        &materials,
        None,
        vfs,
        path,
        texture_resolver,
        force_unique_materials,
        None,
        true, // two_sided
        dynamic_lighting,
        fog_exempt,
        true, // force_alpha_test: leaf cards cast cutout shadows
    );
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
    force_unique_materials: bool,
    bsp_lightmap_tint: Option<[f32; 4]>,
    two_sided: bool,
    dynamic_lighting: bool,
    fog_exempt: bool,
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
    // Preserve *all* UV sets the DFF ships, not just UV0. PAL4 DFFs
    // are single-UV in practice, but a future format/game (or a fan
    // mod) could ship a 2-UV DFF and the lightmap path in
    // `create_geometry_internal` is already wired to consume a
    // secondary set when the caller passes a `bsp_lightmap_tint` and
    // the material carries a `LightMapPlugin`. Dropping UV1 here used
    // to silently disable that path for DFFs.
    let texcoord_sets = if geometry.texcoord_sets.len() >= 1 {
        geometry.texcoord_sets.clone()
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
        // RW stores `SkinPlugin.bone_indices[v][k]` in HAnim *index* space —
        // i.e. it directly addresses the `SkinPlugin.matrix` array (and, by
        // construction, the bone whose HAnim `index` field equals that
        // value). Translate that into our armature slot space via
        // `slot_to_hanim_index`. The `used_bones` field is informational
        // metadata (active-bone subset for GPU matrix uploads) and is *not*
        // an indirection table for per-vertex weighting; treating it as one
        // (as a previous revision did) silently zeroed influences for PAL4
        // skins and froze the mesh in bind pose.
        const SLOT_MISSING: usize = usize::MAX;
        let max_hi = skin
            .matrix
            .len()
            .max(
                hanim_bone
                    .slot_to_hanim_index
                    .iter()
                    .map(|i| *i as usize + 1)
                    .max()
                    .unwrap_or(0),
            )
            .max(256);
        let mut hanim_index_to_slot = vec![SLOT_MISSING; max_hi];
        for (slot, &hi) in hanim_bone.slot_to_hanim_index.iter().enumerate() {
            let hi = hi as usize;
            if hi < hanim_index_to_slot.len() {
                hanim_index_to_slot[hi] = slot;
            }
        }

        let mut remapped_indices: Vec<[u8; 4]> = Vec::with_capacity(skin.bone_indices.len());
        let mut remapped_weights: Vec<[f32; 4]> = Vec::with_capacity(skin.weights.len());
        let mut warned_missing = false;
        for (v, idxs) in skin.bone_indices.iter().enumerate() {
            let mut new_idx = [0u8; 4];
            let mut new_w = skin.weights[v];
            for k in 0..4 {
                let raw = idxs[k] as usize;
                let slot = hanim_index_to_slot
                    .get(raw)
                    .copied()
                    .unwrap_or(SLOT_MISSING);

                if slot == SLOT_MISSING {
                    if !warned_missing {
                        log::warn!(
                            "SkinPlugin: dropping per-vertex bone influence with HAnim index {} (no armature slot)",
                            raw
                        );
                        warned_missing = true;
                    }
                    new_idx[k] = 0;
                    new_w[k] = 0.0;
                } else {
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
        force_unique_materials,
        bsp_lightmap_tint,
        two_sided,
        dynamic_lighting,
        fog_exempt,
        false,
    );
}

pub(crate) fn create_geometry_internal(
    entity: ComRc<IEntity>,
    component_factory: &Rc<dyn ComponentFactory>,
    vertices: &[Vec3f],
    normals: Option<&Vec<Vec3f>>,
    triangles: &[Triangle],
    texcoord_sets: &[Vec<TexCoord>],
    materials: &[Material],
    skin_info: Option<SkinnedMeshInfo>,
    vfs: &MiniFs,
    path: &Path,
    texture_resolver: &dyn TextureResolver,
    force_unique_materials: bool,
    bsp_lightmap_tint: Option<[f32; 4]>,
    two_sided: bool,
    dynamic_lighting: bool,
    fog_exempt: bool,
    force_alpha_test: bool,
) {
    let mut r_vertices = vec![];
    // Forward per-vertex normals only when the geometry actually ships them
    // *and* the caller opts into dynamic lighting; otherwise the mesh stays
    // on the unlit/baked path and the normal attribute is omitted.
    let has_normals = dynamic_lighting && normals.map_or(false, |n| n.len() == vertices.len());
    let mut r_normals: Vec<Vec3> = vec![];
    for i in 0..vertices.len() {
        r_vertices.push(Vec3::new(vertices[i].x, vertices[i].y, vertices[i].z));
        if has_normals {
            let n = &normals.unwrap()[i];
            r_normals.push(Vec3::new(n.x, n.y, n.z));
        }
    }
    let r_normals: Option<&[Vec3]> = if has_normals { Some(&r_normals) } else { None };

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
        let group_idx = match material_to_indices
            .iter()
            .position(|(m, _)| *m == t.material)
        {
            Some(idx) => idx,
            None => {
                let material = &materials[t.material as usize];
                // RenderWare DFF carries blend info implicitly: the
                // material's RGBA `color` alpha byte signals translucency
                // and the texture's own alpha channel separates
                // alpha-cutout (binary) from alpha-blended (graded). See
                // `detect_blend` below.
                let md = if let Some(texture) = material.texture.as_ref() {
                    if let Some(tint) = bsp_lightmap_tint {
                        match load_lightmap_material_pair(
                            material,
                            tint,
                            vfs,
                            path.as_ref(),
                            texture_resolver,
                        ) {
                            Some(md) => md,
                            _ => {
                                // BSP material with a diffuse but no
                                // `LightMapPlugin` (rare PAL4 case). PAL4
                                // BSP diffuses ship with scene-generic
                                // names (`s01`, `s01b`, …) that would
                                // otherwise collide in the process-wide
                                // `TextureStore` LRU across scenes; use
                                // the scoped loader so each scene's
                                // diffuse atlas gets its own
                                // `TextureDef`. See
                                // `load_lightmap_material_pair` for the
                                // same rationale on the lightmap key.
                                load_bsp_material_texture_scoped(
                                    texture,
                                    vfs,
                                    path.as_ref(),
                                    texture_resolver,
                                )
                            }
                        }
                    } else {
                        load_material_texture(
                            texture,
                            vfs,
                            path.as_ref(),
                            texture_resolver,
                            has_normals,
                        )
                    }
                } else if let (Some(tint), Some(_)) =
                    (bsp_lightmap_tint, material.lightmap.as_ref())
                {
                    // PAL4 BSPs occasionally ship materials with NO
                    // primary diffuse `texture` chunk, only a
                    // `LightMapPlugin` (`*LightingMap`) — i.e. the
                    // baked lightmap atlas is the surface's only color
                    // source (typical for cave/wall sectors where the
                    // material was authored as lightmap-only). Build a
                    // `LightMapMaterialDef` with a white dummy diffuse
                    // so `(lightMap * 1.5 * intensity + 0.3) * white *
                    // tint` renders the lightmap straight through.
                    // Without this branch we fell to the `missing`-
                    // texture placeholder below and rendered 1+
                    // fan-shaped pure-black triangle clusters on cave
                    // walls.
                    load_lightmap_only_material(
                        material,
                        tint,
                        vfs,
                        path.as_ref(),
                        texture_resolver,
                    )
                } else {
                    log::debug!("no texture info for material {:?}", path);
                    radiance::rendering::SimpleMaterialDef::create2("missing", None)
                };

                let blend = detect_blend(material, &md);
                let mut md = md.with_blend(blend);
                // PAL5 tree leaves/leaf billboards are graded-alpha (→ AlphaBlend)
                // but must cast leaf-shaped shadows; flag them so the engine routes
                // them through the alpha-clip cutout depth pass. Foliage cards
                // (force_alpha_test) and dynamically-lit blended meshes (PAL5 trees)
                // opt in; opaque/cutout already cast unconditionally.
                if (force_alpha_test || dynamic_lighting) && blend == BlendMode::AlphaBlend {
                    let mut p = *md.params();
                    p.casts_shadow = true;
                    md = md.with_params(p);
                }
                // PAL5 foliage cards are single quads; render them
                // two-sided so the back-facing half is not culled.
                if two_sided && matches!(blend, BlendMode::AlphaTest | BlendMode::AlphaBlend) {
                    md = md.with_cull(CullMode::None);
                }
                if let Some(name) = material.userdata_name.as_deref() {
                    md = md.with_debug_name(name);
                }
                if force_unique_materials {
                    md = md.make_unique();
                }

                // Stamp per-material fog exemption (skybox etc.). Preserves
                // every other param already set on the MaterialDef (lightmap
                // tint/intensity, blend-derived alpha_ref, …).
                if fog_exempt {
                    let mut p = *md.params();
                    p.fog_exempt = true;
                    md = md.with_params(p);
                }

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
            // Per-material vertex layout: a material's shader expects a
            // specific subset of vertex components, and the buffer
            // stride is derived from the texcoord-set count passed to
            // `Geometry::new`. PAL4 BSP sectors carry both UV sets, so
            // the same `r_texcoords` would otherwise force a
            // 2-UV-set stride on every material — including those whose
            // shader (`TexturedNoLight`) only declares one UV input,
            // shifting every vertex attribute and rendering the mesh
            // as garbage. Slice down to the leading UV set for any
            // material whose shader doesn't declare `TEXCOORD2`, and
            // duplicate UV0 into UV1 in the reverse pathological case
            // (lightmap material on a sector that ships only one UV
            // set) so the GPU still reads a well-formed stride.
            let needs_second_uv = matches!(
                v.material.program(),
                radiance::rendering::ShaderProgram::TexturedLightmap
            );
            let single = [r_texcoords[0].clone()];
            let duplicated;
            let geom_texcoords: &[Vec<radiance::components::mesh::TexCoord>] = if needs_second_uv {
                if r_texcoords.len() >= 2 {
                    &r_texcoords
                } else {
                    log::warn!(
                        "[ltmap] lightmap material with only {} UV set(s) for {:?}; duplicating UV0 → UV1 (lightmap will sample diffuse atlas coords and likely look wrong)",
                        r_texcoords.len(),
                        path,
                    );
                    duplicated = [r_texcoords[0].clone(), r_texcoords[0].clone()];
                    &duplicated
                }
            } else if r_texcoords.len() <= 1 {
                &r_texcoords
            } else {
                &single
            };
            radiance::components::mesh::Geometry::new(
                &r_vertices,
                r_normals,
                geom_texcoords,
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
    dynamic_lighting: bool,
) -> MaterialDef {
    let sampler = rw_sampler_def(texture);
    if texture.mask_name.is_empty() {
        let name = texture.name.clone();
        let get_reader = |_name: &str| {
            let data = texture_resolver.resolve_texture(vfs, model_path, &texture.name);
            if data.is_none() {
                log::warn!(
                    "Failed to resolve texture {} for {:?}",
                    texture.name,
                    model_path
                );
            }
            data.and_then(|data| Some(std::io::Cursor::new(data)))
        };
        return if dynamic_lighting {
            radiance::rendering::LitMaterialDef::create_with_sampler(&name, get_reader, sampler)
        } else {
            radiance::rendering::SimpleMaterialDef::create_with_sampler(&name, get_reader, sampler)
        };
    }

    let composite_name = format!("{}|{}", texture.name, texture.mask_name);
    let main_name = texture.name.clone();
    let mask_name = texture.mask_name.clone();
    let model_path_buf = model_path.to_path_buf();
    let composite = {
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
    };
    if dynamic_lighting {
        radiance::rendering::LitMaterialDef::create_with_image_and_sampler(
            &composite_name,
            composite,
            sampler,
        )
    } else {
        radiance::rendering::SimpleMaterialDef::create_with_image_and_sampler(
            &composite_name,
            composite,
            sampler,
        )
    }
}

/// Build a `TextureStore` cache key that is unique per BSP scene/block
/// for a PAL4 BSP-resident texture. PAL4 ships scene-local baked
/// atlases (lightmaps + diffuses) with intentionally generic names
/// (e.g. `Object01LightingMap`, `s01`); the process-wide
/// `TextureStore` LRU is keyed only on the bare name, so two scenes
/// that ship same-named atlases would silently share a single
/// `TextureDef`/GPU texture — UVs correct, atlas wrong. Namespacing
/// the key by the BSP's `model_path` (and a 2-letter slot tag so the
/// lightmap and diffuse can never alias one another) gives each scene
/// its own cache entry. Format:
///     pal4-bsp:<slot>:<model_path>:<bare_name>
/// The `<slot>` tag is one of `"lm"` (lightmap atlas) or `"df"`
/// (diffuse) for `LightMapMaterialDef`, or `"dx"` for the
/// diffuse-only BSP fall-through; pick something stable so a future
/// reader can grep for it.
fn pal4_bsp_texture_key(model_path: &Path, slot: &str, name: &str) -> String {
    format!("pal4-bsp:{}:{}:{}", slot, model_path.display(), name)
}

/// `load_material_texture` for PAL4 BSP diffuse-only materials (the
/// rare BSP case where `material.texture.is_some()` but
/// `material.lightmap.is_none()`). Same `SimpleMaterialDef` shape as
/// `load_material_texture` but the texture cache key is namespaced
/// per-scene via [`pal4_bsp_texture_key`]. Composite (mask) diffuses
/// follow the same scheme for the `composite_name` and the two
/// constituent textures.
fn load_bsp_material_texture_scoped(
    texture: &fileformats::rwbs::material::Texture,
    vfs: &MiniFs,
    model_path: &Path,
    texture_resolver: &dyn TextureResolver,
) -> MaterialDef {
    let sampler = rw_sampler_def(texture);
    if texture.mask_name.is_empty() {
        let bare = texture.name.clone();
        let key = pal4_bsp_texture_key(model_path, "dx", &bare);
        let model_path_buf = model_path.to_path_buf();
        return radiance::rendering::SimpleMaterialDef::create_with_sampler(
            &key,
            move |_name| {
                let data = texture_resolver.resolve_texture(vfs, &model_path_buf, &bare);
                if data.is_none() {
                    log::warn!(
                        "Failed to resolve texture {} for {:?}",
                        bare,
                        model_path_buf
                    );
                }
                data.map(std::io::Cursor::new)
            },
            sampler,
        );
    }

    let main_name = texture.name.clone();
    let mask_name = texture.mask_name.clone();
    let composite_bare = format!("{}|{}", main_name, mask_name);
    let composite_key = pal4_bsp_texture_key(model_path, "dx", &composite_bare);
    let model_path_buf = model_path.to_path_buf();
    radiance::rendering::SimpleMaterialDef::create_with_image_and_sampler(
        &composite_key,
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
/// Build a PAL4 BSP lightmap material: textures = `[lightmap,
/// diffuse]`, shader = `TexturedLightmap`, sampler taken from the BSP
/// material's RW Texture metadata, `MaterialParams.tint` stamped with
/// the scene's `_ltMap.cfg` modulation (`[r, g, b, intensity]`).
///
/// In PAL4 BSPs the lightmap atlas texture name lives in a custom
/// material-level extension (RW chunk type `0x120`) — see
/// `extension::LightMapPlugin`. The material's primary `texture.name`
/// holds the diffuse (e.g. `"fz01"`); the lightmap atlas is e.g.
/// `"Cylinder1746LightingMap"`. Returns `None` when the material does
/// not advertise a lightmap; callers should fall back to the
/// diffuse-only path in that case.
///
/// Sampler & UV channel notes:
/// - The diffuse uses its own sampler (`rw_sampler_def(diffuse)`) — it
///   is usually tiled (`AddressMode::Repeat`) on PAL4 surfaces.
/// - The lightmap atlas **must** clamp on both axes: an atlas's
///   charted regions are surrounded by black bleed, and any sampling
///   outside `[0, 1]` produces the "black tile" artefact that PAL4
///   BSPs are prone to under `Repeat`. We start from the lightmap's
///   own filter mode (parsed from its `0x120` `TEXTURE` chunk) and
///   override the address modes to `Clamp`.
/// - The fragment shader (`lightmap_texture.frag`) samples
///   `texSampler[0]` (lightmap) with the *secondary* UV and
///   `texSampler[1]` (diffuse) with the *primary* UV; the texture
///   vector is therefore `[lightmap, diffuse]`.
fn load_lightmap_material_pair(
    material: &fileformats::rwbs::material::Material,
    tint: [f32; 4],
    vfs: &MiniFs,
    model_path: &Path,
    texture_resolver: &dyn TextureResolver,
) -> Option<MaterialDef> {
    let diffuse = material.texture.as_ref()?;
    let lightmap = material.lightmap.as_ref()?;

    let diffuse_sampler = rw_sampler_def(diffuse);
    let lightmap_sampler = SamplerDef::with_address_uv(
        rw_filter_mode(lightmap.filter_mode),
        AddressMode::Clamp,
        AddressMode::Clamp,
    );
    let model_path_buf = model_path.to_path_buf();

    let mut params = radiance::rendering::MaterialParams::default();
    // `tint[0..3]` is the per-scene RGB modulation from `_ltMap.cfg`;
    // `tint[3]` carries the intensity, which the shader samples from
    // `MaterialParams.misc.y`. The tint vec4's `.a` lane stays at
    // `1.0` so the shader's premultiplied-alpha invariant
    // (`outColor.a = color.a * tint.a`) is preserved.
    params.tint = [tint[0], tint[1], tint[2], 1.0];
    params.intensity = tint[3];

    // PAL4 ships scene-local baked atlases with intentionally generic
    // names (e.g. `Object01LightingMap`, `s01`). `TextureStore` is a
    // process-wide LRU keyed *only* on texture name, so the second
    // scene that mentions a colliding atlas would silently reuse the
    // first scene's pixels — UVs correct, atlas wrong. Namespace both
    // keys by the BSP's `model_path` so each scene gets its own
    // `TextureDef`/GPU texture. The `get_reader` closure receives the
    // scoped key and maps it back to the bare texture name before
    // calling the on-disk resolver (which keys lookup on
    // `model_path.parent() / bare_name`).
    let lightmap_name = lightmap.name.clone();
    let diffuse_name = diffuse.name.clone();
    let lightmap_key = pal4_bsp_texture_key(model_path, "lm", &lightmap_name);
    let diffuse_key = pal4_bsp_texture_key(model_path, "df", &diffuse_name);

    let lightmap_key_for_closure = lightmap_key.clone();
    let md = radiance::rendering::LightMapMaterialDef::create_with_samplers(
        vec![&lightmap_key, &diffuse_key],
        |key| {
            let bare = if key == lightmap_key_for_closure {
                lightmap_name.as_str()
            } else {
                diffuse_name.as_str()
            };
            let data = texture_resolver.resolve_texture(vfs, &model_path_buf, bare);
            if data.is_none() {
                log::warn!(
                    "[ltmap] failed to resolve texture {} for {:?}",
                    bare,
                    model_path_buf
                );
            }
            data.map(std::io::Cursor::new)
        },
        vec![lightmap_sampler, diffuse_sampler],
    );

    Some(md.with_params(params))
}

/// Lightmap-only PAL4 BSP material: same as
/// [`load_lightmap_material_pair`] but the primary diffuse slot is
/// bound to the `radiance_assets` white fallback texture, because the
/// material has no `texture` chunk — its only color information lives
/// in the `LightMapPlugin` atlas. Used for BSP sectors whose material
/// list ships lightmap-only entries (observed in M01 cave walls). The
/// resulting material renders as `(lightMap * 1.5 * intensity + 0.3) *
/// white * tint`, i.e. the baked atlas straight through the per-scene
/// `_ltMap.cfg` modulation. (The ambient floor `+ 0.3` is intentionally
/// kept outside the `intensity` multiply — see
/// `radiance/.../lightmap_texture.frag` for the rationale.)
fn load_lightmap_only_material(
    material: &fileformats::rwbs::material::Material,
    tint: [f32; 4],
    vfs: &MiniFs,
    model_path: &Path,
    texture_resolver: &dyn TextureResolver,
) -> MaterialDef {
    let lightmap = match material.lightmap.as_ref() {
        Some(lm) => lm,
        None => {
            return radiance::rendering::SimpleMaterialDef::create2("missing", None);
        }
    };

    let lightmap_sampler = SamplerDef::with_address_uv(
        rw_filter_mode(lightmap.filter_mode),
        AddressMode::Clamp,
        AddressMode::Clamp,
    );
    // Diffuse slot uses a sentinel name so `LightMapMaterialDef` falls
    // back to `radiance_assets::TEXTURE_WHITE_TEXTURE_FILE` (its
    // built-in white dummy).
    let diffuse_sampler = SamplerDef::default();
    let model_path_buf = model_path.to_path_buf();

    let mut params = radiance::rendering::MaterialParams::default();
    params.tint = [tint[0], tint[1], tint[2], 1.0];
    params.intensity = tint[3];

    // The white-fallback dummy name must NOT alias a real on-disk
    // texture; use a path that's guaranteed not to exist in the VFS so
    // `get_reader` returns `None` and `LightMapMaterialDef::create*`
    // falls back to its built-in white texture asset.
    let white_dummy = "__lightmap_only_white__";

    // Same scoping rationale as `load_lightmap_material_pair`: namespace
    // the lightmap atlas key by `model_path` so per-scene baked atlases
    // with colliding names (M01/1 vs Q01 both ship `Object01LightingMap`)
    // get distinct `TextureDef`s in the global `TextureStore` LRU.
    // The diffuse slot is the shared white-sentinel; it intentionally
    // stays a non-namespaced constant so all lightmap-only materials
    // across all scenes share the single built-in white texture.
    let lightmap_name = lightmap.name.clone();
    let lightmap_key = pal4_bsp_texture_key(model_path, "lm", &lightmap_name);
    let lightmap_key_for_closure = lightmap_key.clone();

    let md = radiance::rendering::LightMapMaterialDef::create_with_samplers(
        vec![&lightmap_key, white_dummy],
        |key| {
            if key == white_dummy {
                return None;
            }
            let bare = if key == lightmap_key_for_closure {
                lightmap_name.as_str()
            } else {
                key
            };
            let data = texture_resolver.resolve_texture(vfs, &model_path_buf, bare);
            data.map(std::io::Cursor::new)
        },
        vec![lightmap_sampler, diffuse_sampler],
    );

    md.with_params(params)
}

fn rw_sampler_def(texture: &fileformats::rwbs::material::Texture) -> SamplerDef {
    SamplerDef::with_address_uv(
        rw_filter_mode(texture.filter_mode),
        rw_address_mode(texture.address_mode_u),
        rw_address_mode(texture.address_mode_v),
    )
}

fn rw_filter_mode(mode: u32) -> FilterMode {
    match mode {
        1 | 3 => FilterMode::Nearest,
        2 | 4 | 5 | 6 => FilterMode::Linear,
        _ => FilterMode::Linear,
    }
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
mod lightmap_cache_key_tests {
    //! PAL4 BSP texture-cache isolation: two scenes that ship same-named
    //! baked atlases (e.g. M01/1 and Q01/Q01 both bundle a
    //! `Object01LightingMap.dds`) must NOT alias one another in the
    //! process-wide `TextureStore` LRU. The atlases are different bakes
    //! and aliasing them produces the well-known "lightmap looks wrong
    //! on part of the mesh" artefact (correct UVs, wrong atlas).
    //!
    //! This is a pure-Rust unit test — no real PAL4 assets required —
    //! using a synthetic in-memory PNG resolver.
    use super::*;
    use fileformats::rwbs::material::{Material as RwMaterial, Texture as RwTexture};
    use std::path::PathBuf;

    /// 1x1 RGBA PNG, encoded once and shared across the synthetic
    /// resolver callbacks. The actual pixel value doesn't matter — only
    /// the cache key does.
    fn one_by_one_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255]));
        let dynamic = image::DynamicImage::ImageRgba8(img);
        let mut buf = std::io::Cursor::new(Vec::new());
        dynamic
            .write_to(&mut buf, image::ImageFormat::Png)
            .expect("encode synthetic png");
        buf.into_inner()
    }

    /// Resolver that always returns the same 1×1 PNG regardless of
    /// path or texture name. Sufficient for cache-key tests because
    /// `LightMapMaterialDef`/`SimpleMaterialDef` only call the
    /// resolver on cache misses; what we're verifying is the key
    /// shape, not the bytes.
    struct ConstResolver(Vec<u8>);
    impl TextureResolver for ConstResolver {
        fn resolve_texture(
            &self,
            _vfs: &MiniFs,
            _model_path: &Path,
            _name: &str,
        ) -> Option<Vec<u8>> {
            Some(self.0.clone())
        }
    }

    fn make_material(diffuse_name: &str, lightmap_name: &str) -> RwMaterial {
        RwMaterial {
            texture: Some(RwTexture {
                name: diffuse_name.to_string(),
                ..RwTexture::default()
            }),
            lightmap: Some(RwTexture {
                name: lightmap_name.to_string(),
                ..RwTexture::default()
            }),
            ..RwMaterial::default()
        }
    }

    #[test]
    fn texture_key_is_unique_per_model_path_and_slot() {
        // Same bare atlas name, different model_path → different keys.
        let a = pal4_bsp_texture_key(
            Path::new("/gamedata/PALWorld/Q01/Q01/Q01.bsp"),
            "lm",
            "Object01LightingMap",
        );
        let b = pal4_bsp_texture_key(
            Path::new("/gamedata/PALWorld/M01/1/1.bsp"),
            "lm",
            "Object01LightingMap",
        );
        assert_ne!(
            a, b,
            "same atlas name across two scenes must hash distinctly"
        );

        // Different slot → different key even at the same model_path.
        let lm = pal4_bsp_texture_key(Path::new("/foo/bar.bsp"), "lm", "s01");
        let df = pal4_bsp_texture_key(Path::new("/foo/bar.bsp"), "df", "s01");
        assert_ne!(lm, df, "slot tag must disambiguate lightmap vs diffuse");

        // Bare name participates verbatim — useful for grep/debug.
        assert!(a.contains("Object01LightingMap"));
        assert!(a.contains("Q01"));
    }

    #[test]
    fn lightmap_pair_uses_per_scene_keys_for_collision() {
        // Two scenes that ship same-named atlases. The MaterialDefs
        // they produce must reference DISTINCT `TextureDef`s (distinct
        // `Arc<TextureDef>` and distinct cache-key strings) so the
        // global `TextureStore` LRU keeps them separate.
        let vfs = MiniFs::new(false);
        let resolver = ConstResolver(one_by_one_png());
        let material = make_material("s01b", "Object01LightingMap");
        let tint = [1.0_f32, 1.0, 1.0, 1.0];

        let path_q01 = PathBuf::from("/gamedata/PALWorld/Q01/Q01/Q01.bsp");
        let path_m01 = PathBuf::from("/gamedata/PALWorld/M01/1/1.bsp");

        let md_q01 = load_lightmap_material_pair(&material, tint, &vfs, &path_q01, &resolver)
            .expect("lightmap pair for Q01");
        let md_m01 = load_lightmap_material_pair(&material, tint, &vfs, &path_m01, &resolver)
            .expect("lightmap pair for M01");

        // Texture order is `[lightmap, diffuse]` per
        // `LightMapMaterialDef::create_with_samplers`.
        assert_eq!(md_q01.textures().len(), 2);
        assert_eq!(md_m01.textures().len(), 2);

        let lm_q01 = md_q01.textures()[0].name();
        let lm_m01 = md_m01.textures()[0].name();
        assert_ne!(
            lm_q01, lm_m01,
            "Q01 and M01 lightmap atlases must have distinct TextureStore keys"
        );
        assert!(
            lm_q01.contains("Q01"),
            "Q01 key should encode its scene path: {}",
            lm_q01
        );
        assert!(
            lm_m01.contains("M01"),
            "M01 key should encode its scene path: {}",
            lm_m01
        );
        assert!(
            lm_q01.contains("Object01LightingMap"),
            "scoped key should still surface the bare atlas name: {}",
            lm_q01,
        );

        // And the diffuse slot is similarly scene-scoped (so `s01b`
        // doesn't collide either).
        let df_q01 = md_q01.textures()[1].name();
        let df_m01 = md_m01.textures()[1].name();
        assert_ne!(df_q01, df_m01, "diffuse keys must also be scene-scoped");

        // Different Arc identity → genuinely separate TextureDef
        // entries in the global store.
        assert!(
            !std::sync::Arc::ptr_eq(&md_q01.textures()[0], &md_m01.textures()[0]),
            "Q01 and M01 lightmap atlases must NOT share an Arc<TextureDef>"
        );
    }
}

#[cfg(test)]
mod hierarchy_tests {
    use super::*;
    use fileformats::rwbs::frame::FRAME_MATRIX_FLAG_HANIM_BONE;

    fn make_test_frame(parent: i32, matrix_flags: u32) -> Frame {
        Frame {
            right: Vec3f {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            up: Vec3f {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            at: Vec3f {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            pos: Vec3f {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            parent,
            matrix_flags,
            extensions: vec![],
        }
    }

    #[test]
    fn frame_attachment_negative_parent_is_clump_root() {
        assert_eq!(frame_attachment(-1, 0), FrameAttachment::ClumpRoot);
        assert_eq!(frame_attachment(-1, 3), FrameAttachment::ClumpRoot);
    }

    #[test]
    fn frame_attachment_self_parent_is_self_parented() {
        assert_eq!(frame_attachment(0, 0), FrameAttachment::SelfParented);
        assert_eq!(frame_attachment(7, 7), FrameAttachment::SelfParented);
    }

    #[test]
    fn frame_attachment_other_parent_is_child() {
        assert_eq!(frame_attachment(2, 5), FrameAttachment::Child(2));
        assert_eq!(frame_attachment(42, 1), FrameAttachment::Child(42));
    }

    #[test]
    fn frame_matrix_flags_bone_bit_round_trips() {
        let bone_frame = make_test_frame(0, FRAME_MATRIX_FLAG_HANIM_BONE);
        let plain_frame = make_test_frame(0, 0);
        assert!(bone_frame.is_hanim_bone());
        assert!(!plain_frame.is_hanim_bone());
    }
}

#[cfg(test)]
mod foliage_tag_tests {
    use super::parse_prt_texture_id;

    #[test]
    fn parses_wind_leaf_texture_id() {
        assert_eq!(parse_prt_texture_id("[w]{t6140}{s60}leaf02"), Some(6140));
        assert_eq!(parse_prt_texture_id("[W]{t6091}{s90}gushuye03"), Some(6091));
        // `{t..}` without a following `{s..}` still parses.
        assert_eq!(parse_prt_texture_id("[w]{t42}leaf"), Some(42));
    }

    #[test]
    fn rejects_non_texture_tags() {
        assert_eq!(parse_prt_texture_id("[($]Plane12"), None);
        assert_eq!(parse_prt_texture_id("objdefault"), None);
        assert_eq!(parse_prt_texture_id("[w]{snan}"), None);
        assert_eq!(parse_prt_texture_id("[w]{t}"), None);
    }
}
