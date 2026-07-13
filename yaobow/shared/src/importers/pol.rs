//! Normalized glTF scene → `.pol` (static, possibly multi-material scenery
//! mesh) converter.
//!
//! Mirrors [`crate::exporters::gltf::pol`] in reverse: every scene-root
//! node with a mesh becomes one [`PolMesh`]; every glTF `Primitive` of
//! that node's mesh becomes one [`PolMaterialInfo`] (POL groups triangles
//! sharing a material, same as the exporter's one-`Primitive`-per-
//! `PolMaterialInfo` mapping), sharing one vertex pool per node (1:1 per
//! glTF vertex, like [`crate::importers::mv3`]). Since the exporter puts
//! every material's `Primitive` for one node on the *same* shared
//! position/normal/UV accessors, primitives whose vertex data is
//! bit-identical to an already-processed primitive of the same node (see
//! [`super::scene::ImportedPrimitive::shares_vertex_data`]) reuse that
//! block instead of appending a duplicate copy of the whole vertex
//! buffer; a primitive with genuinely distinct vertex data (plain,
//! hand-authored glTF) still gets its own appended block.
//!
//! # Supported scene shape (explicit constraints)
//!
//! POL has **no node hierarchy, node transform, or animation** at all —
//! not even MV3's per-vertex morph targets. This converter therefore
//! requires:
//! * every mesh-bearing node must be a **scene root**
//!   ([`ImportError::NestedMeshNode`]);
//! * that node's static scale must be **uniform**
//!   ([`ImportError::NonUniformScale`]) — translation/rotation/scale are
//!   baked directly into the vertex/normal data;
//! * the node must **not** be targeted by any animation channel
//!   ([`ImportError::UnsupportedAnimationTarget`]);
//! * no primitive may have morph targets (POL has no per-vertex animation
//!   at all, unlike MV3) — rejected with [`ImportError::Other`];
//! * every primitive's vertex count must fit in a `u16` index
//!   ([`ImportError::TooManyVertices`]).
//!
//! Round-tripping a model exported by [`crate::exporters::gltf::pol`]
//! recovers `some_flag`/`geom_node_descs`/the optional `unknown_data`
//! block and each material's reserved fields from `asset.extras.yaobow`;
//! plain, hand-authored glTF gets zeroed defaults (`some_flag = 0`, so
//! the optional `unknown_data` block is omitted entirely) and a single
//! `texture_names` entry taken from the primitive's base color texture.

use std::collections::HashSet;
use std::io::{Seek, Write};

use binrw::BinRead;
use fileformats::pol::{
    GeomNodeDesc, PolFile, PolMaterialInfo, PolMesh, PolTriangle, PolVertex, PolVertexComponents,
    UnknownData, write_pol,
};
use fileformats::rwbs::{Matrix44f, TexCoord, Vec3f};
use serde::Deserialize;

use super::error::{Diagnostics, ImportError};
use super::scene::ImportedScene;
use super::target::ImportOptions;

/// Converts `scene` into an in-memory [`PolFile`], applying `options.pol`.
pub fn convert(
    scene: &ImportedScene,
    options: &ImportOptions,
) -> Result<(PolFile, Diagnostics), ImportError> {
    convert_with_template(scene, options, None)
}

/// Like [`convert`], but falls back to `template`'s opaque metadata
/// (`some_flag`/`geom_node_descs`/`unknown_data`/per-material reserved
/// fields) wherever `scene` has no `asset.extras.yaobow` round-trip
/// metadata of its own. `extras`, when present, always takes precedence
/// over `template`.
pub fn convert_with_template(
    scene: &ImportedScene,
    options: &ImportOptions,
    template: Option<&PolFile>,
) -> Result<(PolFile, Diagnostics), ImportError> {
    let mut diagnostics = Diagnostics::default();
    let mut extras = pol_extras(scene, &mut diagnostics);
    if extras.is_none() {
        if let Some(template) = template {
            diagnostics.push(
                "no asset.extras.yaobow round-trip metadata found; falling back to the \
                 replacement template's opaque metadata"
                    .to_string(),
            );
            extras = Some(PolExtras::from_file(template));
        }
    }
    let roots: HashSet<usize> = super::effective_roots(scene).into_iter().collect();

    let mut meshes = Vec::new();
    let mut geom_node_descs = Vec::new();
    for (index, node) in scene.nodes.iter().enumerate() {
        if node.mesh.is_none() {
            continue;
        }
        if !roots.contains(&index) {
            return Err(ImportError::NestedMeshNode {
                node: node.name.clone(),
                target: "pol",
            });
        }
        super::assert_no_trs_animation(scene, index, "pol")?;
        let scale = super::uniform_scale(node, "pol")?;

        let mesh_index = meshes.len();
        let material_metadata = extras
            .as_ref()
            .and_then(|e| e.material_metadata.get(mesh_index));
        meshes.push(build_mesh(
            node,
            options,
            &mut diagnostics,
            scale,
            material_metadata,
        )?);

        geom_node_descs.push(
            extras
                .as_ref()
                .and_then(|e| e.geom_node_descs.get(mesh_index))
                .filter(|d| d.len() == 26)
                .map(|d| GeomNodeDesc { unknown: d.clone() })
                .unwrap_or(GeomNodeDesc {
                    unknown: vec![0u16; 26],
                }),
        );
    }

    if meshes.is_empty() {
        return Err(ImportError::NoGeometry("pol"));
    }

    let some_flag = extras.as_ref().map(|e| e.some_flag).unwrap_or(0);
    let unknown_data: Vec<UnknownData> = if some_flag > 100 {
        match extras.as_ref() {
            Some(e) => e
                .unknown_data
                .iter()
                .cloned()
                .map(PolExtrasUnknownData::try_into_unknown_data)
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        }
    } else {
        Vec::new()
    };

    let file = PolFile {
        some_flag,
        mesh_count: meshes.len() as u32,
        geom_node_descs,
        unknown_count: unknown_data.len() as u32,
        unknown_data,
        meshes,
    };

    Ok((file, diagnostics))
}

/// Converts `scene` and writes the resulting `.pol` bytes to `writer`.
pub fn write(
    scene: &ImportedScene,
    options: &ImportOptions,
    writer: &mut (impl Write + Seek),
) -> Result<Diagnostics, ImportError> {
    let (file, diagnostics) = convert(scene, options)?;
    write_pol(writer, &file)?;
    Ok(diagnostics)
}

/// Builds a [`PolVertexComponents`] bitmask from outside `fileformats`
/// (see the comment at its call site for why this can't be done
/// directly).
fn vertex_components(bits: u32) -> PolVertexComponents {
    let mut cursor = std::io::Cursor::new(bits.to_le_bytes());
    PolVertexComponents::read(&mut cursor)
        .expect("reading 4 bytes from an in-memory buffer cannot fail")
}

fn build_mesh(
    node: &super::scene::ImportedNode,
    options: &ImportOptions,
    diagnostics: &mut Diagnostics,
    scale: f32,
    material_metadata: Option<&Vec<PolExtrasMaterial>>,
) -> Result<PolMesh, ImportError> {
    let mesh = node.mesh.as_ref().expect("caller checked mesh.is_some()");

    let mut pooled_positions: Vec<[f32; 3]> = Vec::new();
    let mut pooled_normals: Vec<Option<[f32; 3]>> = Vec::new();
    let mut pooled_uvs: Vec<[f32; 2]> = Vec::new();
    let mut has_any_normal = false;
    // The base index each already-processed primitive's vertex data
    // starts at in the pool above, parallel to `mesh.primitives` (see
    // `ImportedPrimitive::shares_vertex_data`'s doc comment for why a
    // later primitive may reuse an earlier one's block instead of
    // appending a new one).
    let mut base_index_for_primitive: Vec<u32> = Vec::with_capacity(mesh.primitives.len());

    let mut material_info = Vec::with_capacity(mesh.primitives.len());
    for (prim_index, primitive) in mesh.primitives.iter().enumerate() {
        if !primitive.morph_targets.is_empty() {
            return Err(ImportError::Other(format!(
                "node `{}` primitive #{prim_index} has morph targets, but pol has no per-vertex animation support",
                node.name
            )));
        }

        let vertex_count = primitive.positions.len();
        if vertex_count > u16::MAX as usize + 1 {
            return Err(ImportError::TooManyVertices {
                mesh: node.name.clone(),
                primitive: prim_index,
                count: vertex_count,
            });
        }

        has_any_normal |= !primitive.normals.is_empty();

        let shared_base_index = mesh.primitives[..prim_index]
            .iter()
            .position(|prev| prev.shares_vertex_data(primitive))
            .map(|prev_index| base_index_for_primitive[prev_index]);
        let base_index = match shared_base_index {
            Some(base_index) => base_index,
            None => {
                let base_index = pooled_positions.len() as u32;
                for i in 0..vertex_count {
                    let p = super::quantize_world(primitive.positions[i], node, scale);
                    pooled_positions.push(p);
                    let n = primitive.normals.get(i).copied();
                    pooled_normals.push(n.map(|n| super::rotate_normal(n, node.rotation)));
                    pooled_uvs.push(primitive.uv0.get(i).copied().unwrap_or([0.0, 0.0]));
                }
                base_index
            }
        };
        base_index_for_primitive.push(base_index);

        if primitive.indices.len() % 3 != 0 {
            return Err(ImportError::Other(format!(
                "node `{}` primitive #{prim_index} has {} indices, not a multiple of 3",
                node.name,
                primitive.indices.len()
            )));
        }
        let mut triangles = Vec::with_capacity(primitive.indices.len() / 3);
        for tri in primitive.indices.chunks_exact(3) {
            let mut idx = [0u16; 3];
            for (slot, &raw) in idx.iter_mut().zip(tri) {
                let local =
                    raw.checked_add(base_index)
                        .ok_or_else(|| ImportError::IndexRemapOverflow {
                            mesh: node.name.clone(),
                            primitive: prim_index,
                            index: raw,
                            base_index,
                        })?;
                if local > u16::MAX as u32 {
                    return Err(ImportError::IndexOutOfRange {
                        mesh: node.name.clone(),
                        primitive: prim_index,
                        index: local,
                        vertex_count: pooled_positions.len(),
                    });
                }
                *slot = local as u16;
            }
            triangles.push(PolTriangle { indices: idx });
        }

        let extra = material_metadata.and_then(|m| m.get(prim_index));
        // A Yaobow-extras texture name (when present) takes precedence
        // over the glTF material's texture, since it's the only way to
        // recover the real name for a bufferView-embedded image (see
        // `importers::loader::image_texture_name`'s placeholder). Mirrors
        // the exporter's convention that the *last* `texture_names` entry
        // is the diffuse/base-color slot.
        let texture_name = extra
            .and_then(|e| e.texture_names.last().cloned())
            .or_else(|| primitive.material_texture.clone())
            .unwrap_or_default();
        // GBK-encode (not UTF-8) before the capacity check/construction:
        // `PolMaterialInfo::texture_names` is read back with
        // `StringWithCapacity::as_str` (GBK), so a raw-UTF-8-encoded
        // Chinese name would both mis-measure the 64-byte capacity check
        // below and come back corrupted on read.
        let texture_name_encoded =
            super::gbk_capacity_string(&texture_name, 64, |actual, limit| {
                ImportError::TextureNameTooLong {
                    mesh: node.name.clone(),
                    name: texture_name.clone(),
                    actual,
                    limit,
                }
            })?;
        let use_alpha = if let Some(force) = options.pol.force_use_alpha {
            force as u32
        } else if let Some(extra) = extra {
            extra.use_alpha
        } else {
            primitive.material_alpha_blend as u32
        };
        material_info.push(PolMaterialInfo {
            use_alpha,
            unknown_68: extra
                .filter(|e| e.unknown_68.len() == 16)
                .map(|e| e.unknown_68.clone())
                .unwrap_or_else(|| vec![0.0; 16]),
            unknown_float: extra.map(|e| e.unknown_float).unwrap_or(0.0),
            texture_count: 1,
            texture_names: vec![texture_name_encoded],
            unknown2: extra.map(|e| e.unknown2).unwrap_or(0),
            unknown3: extra.map(|e| e.unknown3).unwrap_or(0),
            unknown4: extra.map(|e| e.unknown4).unwrap_or(0),
            triangle_count: triangles.len() as u32,
            triangles,
        });
    }

    let mut aabb_min = [f32::INFINITY; 3];
    let mut aabb_max = [f32::NEG_INFINITY; 3];
    for p in &pooled_positions {
        for axis in 0..3 {
            aabb_min[axis] = aabb_min[axis].min(p[axis]);
            aabb_max[axis] = aabb_max[axis].max(p[axis]);
        }
    }
    if pooled_positions.is_empty() {
        aabb_min = [0.0; 3];
        aabb_max = [0.0; 3];
        diagnostics.push(format!(
            "node `{}` produced no vertices; emitting an empty mesh",
            node.name
        ));
    }

    // `PolVertexComponents`'s inner `u32` is private and it has no public
    // bitwise-OR/from-bits constructor, so a multi-flag value can't be
    // built directly from its `POSITION`/`NORMAL`/`TEXCOORD` constants
    // from outside `fileformats`. Round-trip the combined bits through
    // `BinRead` (a public API, since the struct is a plain
    // `#[brw(little)]` `u32` newtype) instead.
    let mut vertex_flags: u32 = 0b1 | 0b10000; // POSITION | TEXCOORD
    if has_any_normal {
        vertex_flags |= 0b10; // NORMAL
    }
    let vertex_type = vertex_components(vertex_flags);

    let vertices: Vec<PolVertex> = (0..pooled_positions.len())
        .map(|i| PolVertex {
            position: Vec3f {
                x: pooled_positions[i][0],
                y: pooled_positions[i][1],
                z: pooled_positions[i][2],
            },
            normal: if has_any_normal {
                let n = pooled_normals[i].unwrap_or([0.0, 1.0, 0.0]);
                Some(Vec3f {
                    x: n[0],
                    y: n[1],
                    z: n[2],
                })
            } else {
                None
            },
            unknown4: None,
            unknown8: None,
            tex_coord: TexCoord {
                u: pooled_uvs[i][0],
                v: pooled_uvs[i][1],
            },
            tex_coord2: None,
            unknown40: None,
            unknown80: None,
            unknown100: None,
        })
        .collect();

    Ok(PolMesh {
        aabb_min: Vec3f {
            x: aabb_min[0],
            y: aabb_min[1],
            z: aabb_min[2],
        },
        aabb_max: Vec3f {
            x: aabb_max[0],
            y: aabb_max[1],
            z: aabb_max[2],
        },
        vertex_type,
        vertex_count: vertices.len() as u32,
        vertices,
        material_info_count: material_info.len() as u32,
        material_info,
    })
}

/// Mirrors the material-level fields the exporter emits per material.
/// `texture_names.last()` (the diffuse/base-color slot, matching the
/// exporter's convention) overrides the glTF material's own texture when
/// present — see the round-trip-precedence comment at this struct's use
/// site in [`build_mesh`].
#[derive(Debug, Default, Deserialize)]
struct PolExtrasMaterial {
    #[serde(default)]
    use_alpha: u32,
    #[serde(default)]
    unknown_68: Vec<f32>,
    #[serde(default)]
    unknown_float: f32,
    #[serde(default)]
    texture_names: Vec<String>,
    #[serde(default)]
    unknown2: u32,
    #[serde(default)]
    unknown3: u32,
    #[serde(default)]
    unknown4: u32,
}

/// Mirrors [`fileformats::pol::UnknownData`] (see `importers::mv3`'s
/// shadow-struct comment for why a local `Deserialize`-capable copy is
/// needed instead of the `fileformats` type directly).
#[derive(Debug, Default, Clone, Deserialize)]
struct PolExtrasUnknownData {
    #[serde(default)]
    unknown: Vec<u8>,
    #[serde(default)]
    matrix: [f32; 16],
    #[serde(default)]
    unknown2: u32,
    #[serde(default)]
    ddd_str: String,
}

impl PolExtrasUnknownData {
    /// GBK-encodes `ddd_str` (matching how [`fileformats::pol::UnknownData::ddd_str`]
    /// is decoded on read, via `SizedString::to_string`) instead of the
    /// previous `SizedString::from`'s raw-UTF-8 assumption, which
    /// silently corrupted any non-ASCII (e.g. Chinese) `ddd_str` on the
    /// GBK-decoding read path. Propagates [`ImportError::StringEncoding`]
    /// for a character GBK can't represent, mirroring
    /// [`super::gbk_capacity_string`]'s texture/action-name handling.
    fn try_into_unknown_data(self) -> Result<UnknownData, ImportError> {
        Ok(UnknownData {
            unknown: self.unknown,
            matrix: Matrix44f(self.matrix),
            unknown2: self.unknown2,
            ddd_str: super::gbk_sized_string(&self.ddd_str)?,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct PolExtras {
    #[serde(default)]
    some_flag: u32,
    #[serde(default)]
    geom_node_descs: Vec<Vec<u16>>,
    #[serde(default)]
    unknown_data: Vec<PolExtrasUnknownData>,
    #[serde(default)]
    material_metadata: Vec<Vec<PolExtrasMaterial>>,
}

impl PolExtras {
    /// Builds a fallback "extras" tree directly from a replacement
    /// template's already-parsed [`PolFile`] (see
    /// [`convert_with_template`]), used in place of a real
    /// `asset.extras.yaobow` payload when `scene` has none of its own.
    /// Reads straight off the real on-disk struct fields (no JSON
    /// round trip needed, unlike [`pol_extras`]).
    fn from_file(file: &PolFile) -> Self {
        PolExtras {
            some_flag: file.some_flag,
            geom_node_descs: file
                .geom_node_descs
                .iter()
                .map(|d| d.unknown.clone())
                .collect(),
            unknown_data: file
                .unknown_data
                .iter()
                .map(|u| PolExtrasUnknownData {
                    unknown: u.unknown.clone(),
                    matrix: u.matrix.0,
                    unknown2: u.unknown2,
                    ddd_str: u.ddd_str.to_string().unwrap_or_default(),
                })
                .collect(),
            material_metadata: file
                .meshes
                .iter()
                .map(|mesh| {
                    mesh.material_info
                        .iter()
                        // Mirror `export_pol_to_glb`'s primitive-building
                        // filter (materials with no triangles never
                        // become a `Primitive`) so this template-fallback
                        // metadata stays aligned by `prim_index` too.
                        .filter(|mi| !mi.triangles.is_empty())
                        .map(|mi| PolExtrasMaterial {
                            use_alpha: mi.use_alpha,
                            unknown_68: mi.unknown_68.clone(),
                            unknown_float: mi.unknown_float,
                            texture_names: mi
                                .texture_names
                                .iter()
                                .map(|n| n.as_str().unwrap_or_default())
                                .collect(),
                            unknown2: mi.unknown2,
                            unknown3: mi.unknown3,
                            unknown4: mi.unknown4,
                        })
                        .collect()
                })
                .collect(),
        }
    }
}

fn pol_extras(scene: &ImportedScene, diagnostics: &mut Diagnostics) -> Option<PolExtras> {
    let extras = scene.extras.as_ref()?;
    if extras.target_format() != Some("pol") {
        diagnostics.push(format!(
            "asset.extras.yaobow.payload.target_format is {:?}, not \"pol\"; ignoring round-trip metadata",
            extras.target_format()
        ));
        return None;
    }
    match serde_json::from_value(extras.payload.clone()) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            diagnostics.push(format!(
                "failed to parse asset.extras.yaobow.payload as pol metadata ({err}); using defaults"
            ));
            None
        }
    }
}
