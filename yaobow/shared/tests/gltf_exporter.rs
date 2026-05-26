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
            Mv3Vertex { x: 0, y: 0, z: 0, normal_phi: 0, normal_theta: 0 },
            Mv3Vertex { x: 100 + scale, y: 0, z: 0, normal_phi: 0, normal_theta: 0 },
            Mv3Vertex { x: 0, y: 100 + scale, z: 0, normal_phi: 0, normal_theta: 0 },
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
        v.get("animations").and_then(|a| a.as_array()).map(|a| a.len()).unwrap_or(0) == 0,
        "single-frame mv3 should not emit animations",
    );
    let prim = &v["meshes"][0]["primitives"][0];
    assert!(
        prim.get("targets").and_then(|t| t.as_array()).map(|a| a.len()).unwrap_or(0) == 0,
        "single-frame mv3 should have no morph targets",
    );
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
    assert_eq!(v["scenes"].as_array().unwrap().len(), 1);
    assert_eq!(v["meshes"].as_array().unwrap().len(), 1);
    let prim = &v["meshes"][0]["primitives"][0];
    assert!(prim["attributes"].get("POSITION").is_some());
    assert!(prim["attributes"].get("TEXCOORD_0").is_some());
}
