//! Round-trip tests for the PAL3 glTF exporter.
//!
//! Builds tiny synthetic Mv3File / PolFile structs in memory, runs them
//! through the exporters, and parses the resulting `.glb` JSON chunk
//! directly with `serde_json`. We avoid pulling in the `gltf` reader
//! crate as a dev-dep because it enables the `names` feature on
//! `gltf-json` via Cargo feature unification, which would break the
//! exporter's `Default`-based struct literals.

use std::path::Path;

use fileformats::mv3::{Mv3File, Mv3Frame, Mv3Mesh, Mv3Model, Mv3Triangle, Mv3Vertex};
use fileformats::pol::{
    PolFile, PolMaterialInfo, PolMesh, PolTriangle, PolVertex, PolVertexComponents,
};
use fileformats::rwbs::{TexCoord, Vec3f};
use mini_fs::MiniFs;
use shared::exporters::gltf::{export_mv3_to_glb, export_pol_to_glb};

fn empty_vfs() -> MiniFs {
    MiniFs::new(false)
}

fn vec3f(x: f32, y: f32, z: f32) -> Vec3f {
    Vec3f { x, y, z }
}

/// Pulls the JSON chunk out of a `.glb` blob and parses it. Mirrors the
/// 12-byte glb header + JSON chunk header layout produced by
/// `GlbBuilder::pack`.
fn parse_glb_json(bytes: &[u8]) -> serde_json::Value {
    assert_eq!(&bytes[..4], b"glTF", "missing glTF magic");
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json_type = &bytes[16..20];
    assert_eq!(json_type, b"JSON", "first chunk must be JSON");
    let json_start = 20;
    let json_end = json_start + json_len;
    let mut slice = &bytes[json_start..json_end];
    while slice.last() == Some(&0x20) {
        slice = &slice[..slice.len() - 1];
    }
    serde_json::from_slice(slice).expect("json chunk parses")
}

fn make_mv3(frame_count: u32) -> Mv3File {
    let vertices_for_frame = |scale: i16| {
        vec![
            Mv3Vertex {
                x: 0,
                y: 0,
                z: 0,
                normal_phi: 0,
                normal_theta: 0,
            },
            Mv3Vertex {
                x: 100 + scale,
                y: 0,
                z: 0,
                normal_phi: 0,
                normal_theta: 0,
            },
            Mv3Vertex {
                x: 0,
                y: 100 + scale,
                z: 0,
                normal_phi: 0,
                normal_theta: 0,
            },
        ]
    };
    let frames: Vec<Mv3Frame> = (0..frame_count)
        .map(|i| Mv3Frame {
            timestamp: i * 4580,
            vertices: vertices_for_frame(i as i16 * 10),
        })
        .collect();
    let texcoords = vec![
        TexCoord { u: 0.0, v: 0.0 },
        TexCoord { u: 1.0, v: 0.0 },
        TexCoord { u: 0.0, v: 1.0 },
    ];
    let mesh = Mv3Mesh {
        unknown: 0,
        triangle_count: 1,
        triangles: vec![Mv3Triangle {
            indices: [0, 1, 2],
            texcoord_indices: [0, 1, 2],
        }],
        unknown_data_count: 0,
        unknown_data: vec![],
    };
    let model = Mv3Model {
        unknown: vec![0u8; 64],
        vertex_per_frame: 3,
        aabb_min: [0.0; 3],
        aabb_max: [0.0; 3],
        frame_count,
        frames,
        texcoord_count: 3,
        texcoords,
        mesh_count: 1,
        meshes: vec![mesh],
    };
    Mv3File {
        version: 0,
        duration: 0,
        texture_count: 0,
        unknown_data_count: 0,
        model_count: 1,
        action_count: 0,
        action_desc: vec![],
        unknown_data: vec![],
        textures: vec![],
        models: vec![model],
    }
}

#[test]
fn mv3_exporter_produces_valid_glb_with_morph_targets() {
    let frame_count: u32 = 4;
    let mv3 = make_mv3(frame_count);
    let vfs = empty_vfs();
    let bytes = export_mv3_to_glb(&mv3, &vfs, Path::new("/dummy/x.mv3"))
        .expect("export_mv3_to_glb succeeds");
    let v = parse_glb_json(&bytes);
    assert_eq!(v["asset"]["extras"]["yaobow"]["schema"], 1);
    assert_eq!(
        v["asset"]["extras"]["yaobow"]["payload"]["target_format"],
        "mv3"
    );
    assert_eq!(v["scenes"].as_array().unwrap().len(), 1);
    let meshes = v["meshes"].as_array().unwrap();
    assert_eq!(meshes.len(), 1);
    let prim = &meshes[0]["primitives"][0];
    let targets = prim["targets"].as_array().unwrap();
    assert_eq!(
        targets.len(),
        (frame_count as usize) - 1,
        "one POSITION-delta morph target per non-base frame",
    );
    // Each target must carry a POSITION delta accessor.
    for t in targets {
        assert!(t.get("POSITION").is_some(), "morph target missing POSITION");
    }
    let anims = v["animations"].as_array().unwrap();
    assert_eq!(anims.len(), 1);
    assert!(!anims[0]["channels"].as_array().unwrap().is_empty());
    assert!(!anims[0]["samplers"].as_array().unwrap().is_empty());
}

#[test]
fn mv3_single_frame_exporter_skips_animation() {
    let mv3 = make_mv3(1);
    let vfs = empty_vfs();
    let bytes = export_mv3_to_glb(&mv3, &vfs, Path::new("/dummy/x.mv3"))
        .expect("export_mv3_to_glb succeeds");
    let v = parse_glb_json(&bytes);
    assert!(
        v.get("animations")
            .and_then(|a| a.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
            == 0,
        "single-frame mv3 should not emit animations",
    );
    let prim = &v["meshes"][0]["primitives"][0];
    assert!(
        prim.get("targets")
            .and_then(|t| t.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
            == 0,
        "single-frame mv3 should have no morph targets",
    );
}

/// Builds a single-model, two-mesh `Mv3File` sharing one 6-vertex frame
/// pool (mesh 0 uses verts 0..3, mesh 1 uses verts 3..6) — the shape
/// [`crate::importers::mv3`]'s `group_mv3_nodes` must reassemble back
/// into one `Mv3Model`.
fn make_mv3_two_mesh_model() -> Mv3File {
    let vertex = |x: i16, y: i16| Mv3Vertex {
        x,
        y,
        z: 0,
        normal_phi: 0,
        normal_theta: 0,
    };
    let frame = Mv3Frame {
        timestamp: 0,
        vertices: vec![
            vertex(0, 0),
            vertex(100, 0),
            vertex(0, 100),
            vertex(0, 0),
            vertex(0, 100),
            vertex(100, 100),
        ],
    };
    let texcoords = vec![
        TexCoord { u: 0.0, v: 0.0 },
        TexCoord { u: 1.0, v: 0.0 },
        TexCoord { u: 0.0, v: 1.0 },
        TexCoord { u: 0.0, v: 0.0 },
        TexCoord { u: 0.0, v: 1.0 },
        TexCoord { u: 1.0, v: 1.0 },
    ];
    let mesh0 = Mv3Mesh {
        unknown: 0,
        triangle_count: 1,
        triangles: vec![Mv3Triangle {
            indices: [0, 1, 2],
            texcoord_indices: [0, 1, 2],
        }],
        unknown_data_count: 0,
        unknown_data: vec![],
    };
    let mesh1 = Mv3Mesh {
        unknown: 0,
        triangle_count: 1,
        triangles: vec![Mv3Triangle {
            indices: [3, 4, 5],
            texcoord_indices: [3, 4, 5],
        }],
        unknown_data_count: 0,
        unknown_data: vec![],
    };
    let model = Mv3Model {
        unknown: vec![0u8; 64],
        vertex_per_frame: 6,
        aabb_min: [0.0; 3],
        aabb_max: [1.0; 3],
        frame_count: 1,
        frames: vec![frame],
        texcoord_count: 6,
        texcoords,
        mesh_count: 2,
        meshes: vec![mesh0, mesh1],
    };
    Mv3File {
        version: 0,
        duration: 0,
        texture_count: 0,
        unknown_data_count: 0,
        model_count: 1,
        action_count: 0,
        action_desc: vec![],
        unknown_data: vec![],
        textures: vec![],
        models: vec![model],
    }
}

/// The exporter must emit **one glTF node per (model, mesh) pair** for a
/// multi-mesh model, and tag each with `node.extras.yaobow.payload`
/// recording its original `model_index`/`mesh_index` — this is what lets
/// `importers::mv3::group_mv3_nodes` regroup the sibling nodes back into
/// a single `Mv3Model` (see the full round-trip test in
/// `importers::synthetic_tests`).
#[test]
fn mv3_exporter_tags_multi_mesh_nodes_with_model_and_mesh_index() {
    let mv3 = make_mv3_two_mesh_model();
    let vfs = empty_vfs();
    let bytes = export_mv3_to_glb(&mv3, &vfs, Path::new("/dummy/multi.mv3"))
        .expect("export_mv3_to_glb succeeds");
    let v = parse_glb_json(&bytes);

    let nodes = v["nodes"].as_array().unwrap();
    let mesh_nodes: Vec<&serde_json::Value> =
        nodes.iter().filter(|n| n.get("mesh").is_some()).collect();
    assert_eq!(mesh_nodes.len(), 2, "one node per (model, mesh) pair");

    let mut tags: Vec<(u64, u64)> = mesh_nodes
        .iter()
        .map(|n| {
            let payload = &n["extras"]["yaobow"]["payload"];
            (
                payload["model_index"].as_u64().unwrap(),
                payload["mesh_index"].as_u64().unwrap(),
            )
        })
        .collect();
    tags.sort();
    assert_eq!(tags, vec![(0, 0), (0, 1)]);
}

fn pol_vertex_components_pos_uv() -> PolVertexComponents {
    use fileformats::binrw::BinRead;
    use std::io::Cursor;
    // POSITION (0b1) | TEXCOORD (0b10000) — bits straight from pol.rs.
    let bits: u32 = 0b1 | 0b10000;
    PolVertexComponents::read(&mut Cursor::new(bits.to_le_bytes())).unwrap()
}

fn make_pol() -> PolFile {
    let vertices = vec![
        PolVertex {
            position: vec3f(0.0, 0.0, 0.0),
            normal: None,
            unknown4: None,
            unknown8: None,
            tex_coord: TexCoord { u: 0.0, v: 0.0 },
            tex_coord2: None,
            unknown40: None,
            unknown80: None,
            unknown100: None,
        },
        PolVertex {
            position: vec3f(1.0, 0.0, 0.0),
            normal: None,
            unknown4: None,
            unknown8: None,
            tex_coord: TexCoord { u: 1.0, v: 0.0 },
            tex_coord2: None,
            unknown40: None,
            unknown80: None,
            unknown100: None,
        },
        PolVertex {
            position: vec3f(0.0, 1.0, 0.0),
            normal: None,
            unknown4: None,
            unknown8: None,
            tex_coord: TexCoord { u: 0.0, v: 1.0 },
            tex_coord2: None,
            unknown40: None,
            unknown80: None,
            unknown100: None,
        },
    ];
    let material_info = PolMaterialInfo {
        use_alpha: 0,
        unknown_68: vec![0.0; 16],
        unknown_float: 0.0,
        texture_count: 0,
        texture_names: vec![],
        unknown2: 0,
        unknown3: 0,
        unknown4: 0,
        triangle_count: 1,
        triangles: vec![PolTriangle { indices: [0, 1, 2] }],
    };
    let mesh = PolMesh {
        aabb_min: vec3f(0.0, 0.0, 0.0),
        aabb_max: vec3f(0.0, 0.0, 0.0),
        vertex_type: pol_vertex_components_pos_uv(),
        vertex_count: 3,
        vertices,
        material_info_count: 1,
        material_info: vec![material_info],
    };
    PolFile {
        some_flag: 100,
        mesh_count: 1,
        geom_node_descs: vec![fileformats::pol::GeomNodeDesc {
            unknown: vec![0u16; 26],
        }],
        unknown_count: 0,
        unknown_data: vec![],
        meshes: vec![mesh],
    }
}

#[test]
fn pol_exporter_produces_valid_glb() {
    let pol = make_pol();
    let vfs = empty_vfs();
    let bytes = export_pol_to_glb(&pol, &vfs, Path::new("/dummy/x.pol"))
        .expect("export_pol_to_glb succeeds");
    let v = parse_glb_json(&bytes);
    assert_eq!(
        v["asset"]["extras"]["yaobow"]["payload"]["target_format"],
        "pol"
    );
    assert_eq!(v["scenes"].as_array().unwrap().len(), 1);
    assert_eq!(v["meshes"].as_array().unwrap().len(), 1);
    let prim = &v["meshes"][0]["primitives"][0];
    assert!(prim["attributes"].get("POSITION").is_some());
    assert!(prim["attributes"].get("TEXCOORD_0").is_some());
}

/// A `PolMesh` whose **first** material has zero triangles (skipped by
/// the exporter's primitive-building loop) and whose **second** material
/// is the only one with real geometry: `asset.extras.yaobow.payload
/// .material_metadata[mesh_index]` must contain **exactly one** entry
/// (the non-empty material's), aligned with the single emitted
/// `Primitive` — not two entries with the empty material's (all-default)
/// metadata still occupying index 0.
#[test]
fn pol_exporter_filters_material_metadata_to_match_emitted_primitives() {
    let vertices = vec![
        PolVertex {
            position: vec3f(0.0, 0.0, 0.0),
            normal: None,
            unknown4: None,
            unknown8: None,
            tex_coord: TexCoord { u: 0.0, v: 0.0 },
            tex_coord2: None,
            unknown40: None,
            unknown80: None,
            unknown100: None,
        },
        PolVertex {
            position: vec3f(1.0, 0.0, 0.0),
            normal: None,
            unknown4: None,
            unknown8: None,
            tex_coord: TexCoord { u: 1.0, v: 0.0 },
            tex_coord2: None,
            unknown40: None,
            unknown80: None,
            unknown100: None,
        },
        PolVertex {
            position: vec3f(0.0, 1.0, 0.0),
            normal: None,
            unknown4: None,
            unknown8: None,
            tex_coord: TexCoord { u: 0.0, v: 1.0 },
            tex_coord2: None,
            unknown40: None,
            unknown80: None,
            unknown100: None,
        },
    ];
    let empty_material = PolMaterialInfo {
        use_alpha: 0,
        unknown_68: vec![0.0; 16],
        unknown_float: 0.0,
        texture_count: 0,
        texture_names: vec![],
        unknown2: 0,
        unknown3: 0,
        unknown4: 0,
        triangle_count: 0,
        triangles: vec![],
    };
    let real_material = PolMaterialInfo {
        use_alpha: 1,
        unknown_68: vec![9.5; 16],
        unknown_float: 3.25,
        texture_count: 1,
        texture_names: vec!["second.bmp".into()],
        unknown2: 42,
        unknown3: 43,
        unknown4: 44,
        triangle_count: 1,
        triangles: vec![PolTriangle { indices: [0, 1, 2] }],
    };
    let mesh = PolMesh {
        aabb_min: vec3f(0.0, 0.0, 0.0),
        aabb_max: vec3f(1.0, 1.0, 0.0),
        vertex_type: pol_vertex_components_pos_uv(),
        vertex_count: 3,
        vertices,
        material_info_count: 2,
        material_info: vec![empty_material, real_material],
    };
    let pol = PolFile {
        some_flag: 0,
        mesh_count: 1,
        geom_node_descs: vec![fileformats::pol::GeomNodeDesc {
            unknown: vec![0u16; 26],
        }],
        unknown_count: 0,
        unknown_data: vec![],
        meshes: vec![mesh],
    };

    let vfs = empty_vfs();
    let bytes = export_pol_to_glb(&pol, &vfs, Path::new("/dummy/x.pol"))
        .expect("export_pol_to_glb succeeds");
    let v = parse_glb_json(&bytes);

    // Only one primitive is emitted (the empty material is skipped).
    let prims = v["meshes"][0]["primitives"].as_array().unwrap();
    assert_eq!(prims.len(), 1);

    // `material_metadata` for this mesh must match that same count and
    // carry the *real* material's data at index 0, not the empty one's.
    let mesh_metadata = v["asset"]["extras"]["yaobow"]["payload"]["material_metadata"][0]
        .as_array()
        .unwrap();
    assert_eq!(mesh_metadata.len(), 1);
    assert_eq!(mesh_metadata[0]["unknown2"], 42);
    assert_eq!(mesh_metadata[0]["unknown3"], 43);
    assert_eq!(mesh_metadata[0]["unknown4"], 44);
    assert_eq!(mesh_metadata[0]["texture_names"][0], "second.bmp");
}
