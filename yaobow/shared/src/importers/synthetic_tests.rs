//! Synthetic-glTF tests for the normalized importer + MV3/POL/CVD
//! converters.
//!
//! Every test builds its input in-memory via [`super::test_support::SceneBuilder`]
//! (no on-disk fixtures, no external tooling) and drives the same
//! [`super::load_gltf_scene_from`] / `convert`/`convert_with_template` /
//! `write_*` functions the CLI-level [`super::convert_gltf_to_bytes`] uses.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use fileformats::mv3::{
    Mv3ActionDesc, Mv3File, Mv3Frame, Mv3Mesh, Mv3Model, Mv3Texture, Mv3Triangle, Mv3Vertex,
};
use fileformats::rwbs::TexCoord;
use fileformats::utils::SizedString;
use mini_fs::MiniFs;

use super::error::ImportError;
use super::scene::TrsProperty;
use super::target::{ImportOptions, TargetFormat};
use super::test_support::SceneBuilder;
use super::{convert_gltf_to_bytes, cvd, load_gltf_scene_from, mv3, pol};
use crate::exporters::gltf::export_mv3_to_glb;

/// A unit square (two triangles) with a flat +Y normal and a simple UV
/// layout, shared by several tests that just need "some" valid geometry.
fn quad() -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u16>) {
    let positions = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];
    let normals = vec![[0.0, 1.0, 0.0]; 4];
    let uv0 = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let indices = vec![0u16, 1, 2, 0, 2, 3];
    (positions, normals, uv0, indices)
}

fn one_pixel_png() -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        1,
        1,
        image::Rgba([10, 20, 30, 128]),
    ))
    .write_to(&mut out, image::ImageOutputFormat::Png)
    .unwrap();
    out.into_inner()
}

fn scratch_named_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "openpal3-importers-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Generic (no `asset.extras.yaobow`) static glTF -> POL: parse, convert,
/// write, then read the result back with `fileformats::pol::read_pol`
/// and confirm geometry/material data round-tripped.
#[test]
fn generic_gltf_to_pol_parse_write_read() {
    let dir = scratch_named_dir("generic-pol");
    std::fs::write(dir.join("diffuse.png"), one_pixel_png()).unwrap();
    let (positions, normals, uv0, indices) = quad();
    let mut builder = SceneBuilder::new();
    let image = builder.add_image_uri("diffuse.png");
    let texture = builder.add_texture(image);
    let material = builder.add_material(Some(texture), false);
    let mesh = builder.add_triangle_mesh(
        &positions,
        Some(&normals),
        &uv0,
        &indices,
        Some(material),
        &[],
    );
    let node = builder.add_node(Some(mesh), &[], None, None, None);
    let gltf = builder.parse(&[node]);

    let (scene, diagnostics) = load_gltf_scene_from(&gltf, &dir).expect("load");
    assert!(
        diagnostics.messages().any(|m| m.contains("diffuse.tga")),
        "expected texture conversion diagnostic: {:?}",
        diagnostics
    );
    assert!(scene.extras.is_none());

    let (pol_file, _diag) = pol::convert(&scene, &ImportOptions::default()).expect("convert");
    assert_eq!(pol_file.meshes.len(), 1);
    let mesh0 = &pol_file.meshes[0];
    assert_eq!(mesh0.vertex_count, 4);
    assert_eq!(mesh0.material_info.len(), 1);
    assert_eq!(
        mesh0.material_info[0].texture_names[0].as_str().unwrap(),
        "_yaobow_import/diffuse.tga"
    );

    let mut bytes = Vec::new();
    pol::write(
        &scene,
        &ImportOptions::default(),
        &mut Cursor::new(&mut bytes),
    )
    .expect("write");

    let read_back = fileformats::pol::read_pol(&mut Cursor::new(&bytes)).expect("read back");
    assert_eq!(read_back.meshes.len(), 1);
    assert_eq!(read_back.meshes[0].vertex_count, 4);
    assert_eq!(read_back.meshes[0].vertices.len(), 4);
    assert_eq!(
        read_back.meshes[0].material_info[0].texture_names[0]
            .as_str()
            .unwrap(),
        "_yaobow_import/diffuse.tga"
    );
}

#[test]
fn image_data_uri_is_converted_to_tga_with_alpha() {
    let (positions, normals, uv0, indices) = quad();
    let uri = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(one_pixel_png())
    );
    let mut builder = SceneBuilder::new();
    let image = builder.add_image_uri(&uri);
    let texture = builder.add_texture(image);
    let material = builder.add_material(Some(texture), true);
    let mesh = builder.add_triangle_mesh(
        &positions,
        Some(&normals),
        &uv0,
        &indices,
        Some(material),
        &[],
    );
    let node = builder.add_node(Some(mesh), &[], None, None, None);
    let gltf = builder.parse(&[node]);

    let (scene, _) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load data URI");
    assert_eq!(scene.textures.len(), 1);
    assert_eq!(
        scene.textures[0].relative_path,
        "_yaobow_import/embedded_image_0.tga"
    );
    let decoded =
        image::load_from_memory_with_format(&scene.textures[0].bytes, image::ImageFormat::Tga)
            .unwrap()
            .to_rgba8();
    assert_eq!(decoded.get_pixel(0, 0).0[3], 128);
}

#[test]
fn repeated_images_deduplicate_and_colliding_basenames_are_suffixed() {
    let dir = scratch_named_dir("texture-names");
    std::fs::create_dir_all(dir.join("a")).unwrap();
    std::fs::create_dir_all(dir.join("b")).unwrap();
    std::fs::write(dir.join("a/shared.png"), one_pixel_png()).unwrap();
    std::fs::write(dir.join("b/shared.png"), one_pixel_png()).unwrap();

    let (positions, normals, uv0, indices) = quad();
    let mut builder = SceneBuilder::new();
    let image_a = builder.add_image_uri("a/shared.png");
    let image_b = builder.add_image_uri("b/shared.png");
    let texture_a = builder.add_texture(image_a);
    let texture_b = builder.add_texture(image_b);
    let material_a = builder.add_material(Some(texture_a), false);
    let material_b = builder.add_material(Some(texture_b), false);
    let mesh_a = builder.add_triangle_mesh(
        &positions,
        Some(&normals),
        &uv0,
        &indices,
        Some(material_a),
        &[],
    );
    let mesh_a_again = builder.add_triangle_mesh(
        &positions,
        Some(&normals),
        &uv0,
        &indices,
        Some(material_a),
        &[],
    );
    let mesh_b = builder.add_triangle_mesh(
        &positions,
        Some(&normals),
        &uv0,
        &indices,
        Some(material_b),
        &[],
    );
    let nodes = [
        builder.add_node(Some(mesh_a), &[], None, None, None),
        builder.add_node(Some(mesh_a_again), &[], None, None, None),
        builder.add_node(Some(mesh_b), &[], None, None, None),
    ];
    let gltf = builder.parse(&nodes);

    let (scene, _) = load_gltf_scene_from(&gltf, &dir).expect("load textures");
    assert_eq!(scene.textures.len(), 2);
    assert_eq!(scene.textures[0].relative_path, "_yaobow_import/shared.tga");
    assert_eq!(
        scene.textures[1].relative_path,
        "_yaobow_import/shared_2.tga"
    );
}

/// Yaobow-exported MV3 GLB -> import round trip, including a
/// bufferView-embedded texture: the loader extracts it as TGA and that real
/// imported artifact takes precedence over the round-trip texture name.
#[test]
fn mv3_embedded_image_yaobow_extras_round_trip() {
    let (positions, normals, uv0, indices) = quad();
    let morph_deltas = vec![[0.0, 0.5, 0.0]; 4]; // one morph target: lift all verts by 0.5 on Y

    let mut builder = SceneBuilder::new();
    let embedded_bytes = one_pixel_png();
    let image = builder.add_image_embedded(&embedded_bytes, "image/png");
    let texture = builder.add_texture(image);
    let material = builder.add_material(Some(texture), false);
    let mesh = builder.add_triangle_mesh(
        &positions,
        Some(&normals),
        &uv0,
        &indices,
        Some(material),
        std::slice::from_ref(&morph_deltas),
    );
    let node = builder.add_node(Some(mesh), &[], None, None, None);
    builder.add_weights_animation(node, &[0.0, 1.0], &[0.0, 1.0]);
    builder.set_asset_extras(serde_json::json!({
        "yaobow": {
            "schema": 1,
            "payload": {
                "target_format": "mv3",
                "version": 4,
                "action_desc": [{ "tick": 10, "name": "walk" }],
                "textures": [{
                    "unknown": vec![0.0f32; 17],
                    "names": ["real_diffuse.png", "", "", ""],
                }],
                "file_unknown_data": [],
                "model_metadata": [],
            }
        }
    }));
    let gltf = builder.parse(&[node]);

    let (scene, diagnostics) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
    assert!(
        diagnostics
            .messages()
            .any(|m| m.contains("embedded_image_0.tga")),
        "expected an embedded-image conversion diagnostic, got: {:?}",
        diagnostics
    );
    assert!(scene.extras.is_some());

    let (mv3_file, _diag) = mv3::convert(&scene, &ImportOptions::default()).expect("convert");
    assert_eq!(mv3_file.models.len(), 1);
    // 1 morph target => 2 frames (base + target).
    assert_eq!(mv3_file.models[0].frame_count, 2);
    assert_eq!(mv3_file.action_desc.len(), 1);
    assert_eq!(mv3_file.action_desc[0].name.as_str().unwrap(), "walk");
    assert_eq!(
        mv3_file.textures[0].names[0].to_string().unwrap(),
        "_yaobow_import/embedded_image_0.tga"
    );

    let mut bytes = Vec::new();
    fileformats::mv3::write_mv3(&mut Cursor::new(&mut bytes), &mv3_file).expect("write");
    let read_back = fileformats::mv3::read_mv3(&mut Cursor::new(&bytes)).expect("read back");
    assert_eq!(read_back.models[0].frame_count, 2);
    assert_eq!(
        read_back.textures[0].names[0].to_string().unwrap(),
        "_yaobow_import/embedded_image_0.tga"
    );
}

/// Full export -> import round trip for a real MV3 file (not synthetic
/// glTF) whose single model has **two meshes**: drives the actual
/// [`export_mv3_to_glb`] exporter, re-parses its `.glb` output with
/// `gltf::Gltf::from_slice` (the same entry point `load_gltf_scene_from`
/// uses in production), and asserts [`mv3::convert`] regroups the two
/// per-mesh sibling nodes the exporter emits back into **one**
/// `Mv3Model` with two meshes — not two separate models. Also checks
/// texture assignment and per-mesh "unknown" metadata survive the round
/// trip via `node.extras.yaobow` grouping (see
/// `importers::mv3::group_mv3_nodes`).
#[test]
fn mv3_multi_mesh_model_round_trip_preserves_grouping() {
    let vertex = |x: i16, y: i16, z: i16| Mv3Vertex {
        x,
        y,
        z,
        normal_phi: 0,
        normal_theta: 0,
    };
    // One shared per-model vertex/texcoord pool (6 verts), split into two
    // non-overlapping triangles - mesh 0 uses verts 0..3, mesh 1 uses
    // verts 3..6, matching how a real multi-mesh mv3 model shares one
    // frame pool across all of its meshes.
    let frame = Mv3Frame {
        timestamp: 0,
        vertices: vec![
            vertex(0, 0, 0),
            vertex(100, 0, 0),
            vertex(0, 100, 0),
            vertex(0, 0, 0),
            vertex(0, 100, 0),
            vertex(100, 100, 0),
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
        unknown: 111,
        triangle_count: 1,
        triangles: vec![Mv3Triangle {
            indices: [0, 1, 2],
            texcoord_indices: [0, 1, 2],
        }],
        unknown_data_count: 0,
        unknown_data: vec![],
    };
    let mesh1 = Mv3Mesh {
        unknown: 222,
        triangle_count: 1,
        triangles: vec![Mv3Triangle {
            indices: [3, 4, 5],
            texcoord_indices: [3, 4, 5],
        }],
        unknown_data_count: 0,
        unknown_data: vec![],
    };
    let model = Mv3Model {
        unknown: vec![7u8; 64],
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
    let mv3 = Mv3File {
        version: 4,
        duration: 0,
        texture_count: 1,
        unknown_data_count: 0,
        model_count: 1,
        action_count: 0,
        action_desc: vec![],
        unknown_data: vec![],
        textures: vec![Mv3Texture {
            unknown: vec![0.0; 17],
            names: vec![
                SizedString::from("body.bmp"),
                SizedString::from(""),
                SizedString::from(""),
                SizedString::from(""),
            ],
        }],
        models: vec![model],
    };

    let vfs = MiniFs::new(false);
    let glb_bytes = export_mv3_to_glb(&mv3, &vfs, Path::new("/dummy/role.mv3"))
        .expect("export_mv3_to_glb succeeds");

    // Re-parse exactly like the production glTF loader does.
    let gltf = gltf::Gltf::from_slice(&glb_bytes).expect("re-parsing exported glb");
    let (scene, diagnostics) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        diagnostics
    );
    // The exporter emits one node per (model, mesh): two sibling root
    // nodes for our single two-mesh model.
    assert_eq!(scene.nodes.iter().filter(|n| n.mesh.is_some()).count(), 2);

    let (mv3_file, convert_diag) =
        mv3::convert(&scene, &ImportOptions::default()).expect("convert");
    assert!(
        convert_diag.is_empty(),
        "unexpected conversion diagnostics: {:?}",
        convert_diag
    );

    // One model, not two - grouping recovered the original structure.
    assert_eq!(mv3_file.models.len(), 1, "expected exactly one Mv3Model");
    let model = &mv3_file.models[0];
    assert_eq!(model.mesh_count, 2);
    assert_eq!(model.meshes.len(), 2);

    // Per-mesh metadata (`unknown`) round-tripped positionally.
    assert_eq!(model.meshes[0].unknown, 111);
    assert_eq!(model.meshes[1].unknown, 222);
    // Per-model metadata (`unknown`, the 64-byte reserved block).
    assert_eq!(model.unknown, vec![7u8; 64]);

    // Both meshes still reference triangles into one shared vertex pool.
    assert_eq!(model.vertex_per_frame, 6);
    assert_eq!(model.frame_count, 1);

    // Texture assignment: the model's texture metadata (name) survived.
    assert_eq!(mv3_file.textures.len(), 1);
    assert_eq!(
        mv3_file.textures[0].names[0].to_string().unwrap(),
        "body.bmp"
    );

    // Full write -> read-back round trip through the raw binary format too.
    let mut bytes = Vec::new();
    fileformats::mv3::write_mv3(&mut Cursor::new(&mut bytes), &mv3_file).expect("write");
    let read_back = fileformats::mv3::read_mv3(&mut Cursor::new(&bytes)).expect("read back");
    assert_eq!(read_back.models.len(), 1);
    assert_eq!(read_back.models[0].meshes.len(), 2);
    assert_eq!(read_back.models[0].meshes[0].unknown, 111);
    assert_eq!(read_back.models[0].meshes[1].unknown, 222);
    assert_eq!(
        read_back.textures[0].names[0].to_string().unwrap(),
        "body.bmp"
    );
}

/// A vertex position large enough that, after `Mv3Options::vertex_scale`,
/// it overflows the i16 quantization range must be a hard error, not a
/// silently-wrapped value.
#[test]
fn mv3_quantization_overflow_is_an_error() {
    let positions = vec![
        [30000.0, 0.0, 0.0],
        [30000.0, 0.0, 1.0],
        [30001.0, 0.0, 0.0],
    ];
    let uv0 = vec![[0.0, 0.0]; 3];
    let indices = vec![0u16, 1, 2];

    let mut builder = SceneBuilder::new();
    let mesh = builder.add_triangle_mesh(&positions, None, &uv0, &indices, None, &[]);
    let node = builder.add_node(Some(mesh), &[], None, None, None);
    let gltf = builder.parse(&[node]);

    let (scene, _diag) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
    let err = mv3::convert(&scene, &ImportOptions::default()).unwrap_err();
    assert!(
        matches!(err, ImportError::QuantizationOverflow { .. }),
        "expected QuantizationOverflow, got {err:?}"
    );
}

/// A non-`TRIANGLES` primitive topology must be rejected at load time,
/// before any target-format conversion runs.
#[test]
fn unsupported_topology_is_rejected_at_load() {
    let (positions, _normals, uv0, indices) = quad();
    let mut builder = SceneBuilder::new();
    // mode 1 = LINES.
    let mesh = builder.add_mesh_with_mode(&positions, &uv0, &indices, 1);
    let node = builder.add_node(Some(mesh), &[], None, None, None);
    let gltf = builder.parse(&[node]);

    let err = load_gltf_scene_from(&gltf, Path::new(".")).unwrap_err();
    assert!(
        matches!(err, ImportError::UnsupportedTopology { .. }),
        "expected UnsupportedTopology, got {err:?}"
    );
}

/// `CUBICSPLINE` tangents have no representation in the normalized IR, but the
/// key values can still be imported lossily as LINEAR animation.
#[test]
fn cubic_spline_animation_is_imported_lossily() {
    let (positions, _normals, uv0, indices) = quad();
    let mut builder = SceneBuilder::new();
    let mesh = builder.add_triangle_mesh(&positions, None, &uv0, &indices, None, &[]);
    let node = builder.add_node(Some(mesh), &[], None, None, None);
    builder.add_trs_animation(
        node,
        "rotation",
        &[0.0, 1.0],
        &[
            0.0, 0.0, 0.0, 0.0, // key 0 in tangent
            0.0, 0.0, 0.0, 1.0, // key 0 value
            0.0, 0.0, 0.0, 0.0, // key 0 out tangent
            0.0, 0.0, 0.0, 0.0, // key 1 in tangent
            0.0, 0.0, 1.0, 0.0, // key 1 value
            0.0, 0.0, 0.0, 0.0, // key 1 out tangent
        ],
        "CUBICSPLINE",
    );
    let gltf = builder.parse(&[node]);

    let (scene, diagnostics) = load_gltf_scene_from(&gltf, Path::new(".")).expect("lossy load");
    assert!(
        diagnostics
            .messages()
            .any(|message| message.contains("dropped tangents")),
        "expected cubic-spline diagnostic: {diagnostics:?}"
    );
    let channel = &scene.animations[0].trs_channels[0];
    assert_eq!(channel.interpolation, super::Interpolation::Linear);
    assert_eq!(channel.values.len(), 2);
    assert_eq!(channel.values[0], [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(channel.values[1], [0.0, 0.0, 1.0, 0.0]);
}

/// POL cannot represent animated node transforms, while MV3 can bake them into
/// its per-vertex frame snapshots.
#[test]
fn mv3_bakes_animated_mesh_node_transform_while_pol_rejects() {
    let (positions, normals, uv0, indices) = quad();
    let mut builder = SceneBuilder::new();
    let mesh = builder.add_triangle_mesh(&positions, Some(&normals), &uv0, &indices, None, &[]);
    let node = builder.add_node(Some(mesh), &[], None, None, None);
    builder.add_trs_animation(
        node,
        "translation",
        &[0.0, 1.0],
        &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        "LINEAR",
    );
    let gltf = builder.parse(&[node]);

    let (scene, _diag) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");

    let pol_err = pol::convert(&scene, &ImportOptions::default()).unwrap_err();
    assert!(matches!(
        pol_err,
        ImportError::UnsupportedAnimationTarget { .. }
    ));

    let mut options = ImportOptions::default();
    options.mv3.vertex_scale = 1.0;
    options.mv3.ticks_per_second = 100.0;
    let (mv3_file, _) = mv3::convert(&scene, &options).expect("mv3 bake");
    let model = &mv3_file.models[0];
    assert_eq!(model.frame_count, 2);
    assert_eq!(model.frames[0].timestamp, 0);
    assert_eq!(model.frames[1].timestamp, 100);
    assert_eq!(model.frames[0].vertices[0].x, 0);
    assert_eq!(model.frames[1].vertices[0].x, 1);
}

#[test]
fn mv3_flattens_nested_static_node_hierarchy() {
    let (positions, normals, uv0, indices) = quad();
    let mut builder = SceneBuilder::new();
    let mesh = builder.add_triangle_mesh(&positions, Some(&normals), &uv0, &indices, None, &[]);
    let child = builder.add_node(Some(mesh), &[], Some([1.0, 0.0, 0.0]), None, None);
    let half = std::f32::consts::FRAC_PI_4;
    let parent = builder.add_node(
        None,
        &[child],
        Some([2.0, 0.0, 0.0]),
        Some([0.0, 0.0, half.sin(), half.cos()]),
        None,
    );
    let gltf = builder.parse(&[parent]);
    let (scene, _) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");

    let mut options = ImportOptions::default();
    options.mv3.vertex_scale = 1.0;
    let (mv3_file, _) = mv3::convert(&scene, &options).expect("flatten hierarchy");
    assert_eq!(mv3_file.models[0].frame_count, 1);
    assert_eq!(mv3_file.models[0].frames[0].vertices[0].x, 2);
    assert_eq!(mv3_file.models[0].frames[0].vertices[0].y, 1);
}

#[test]
fn mv3_bakes_skeletal_animation_to_vertex_frames() {
    let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let normals = [[0.0, 0.0, 1.0]; 3];
    let uv0 = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
    let mut builder = SceneBuilder::new();
    let mesh = builder.add_triangle_mesh(&positions, Some(&normals), &uv0, &[0, 1, 2], None, &[]);
    builder.set_mesh_skin_attributes(mesh, &[[0, 0, 0, 0]; 3], &[[1.0, 0.0, 0.0, 0.0]; 3]);
    let joint = builder.add_node(None, &[], None, None, None);
    let mesh_node = builder.add_node(Some(mesh), &[], None, None, None);
    let parent = builder.add_node(
        None,
        &[joint, mesh_node],
        Some([10.0, 0.0, 0.0]),
        None,
        None,
    );
    let inverse_bind = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-10.0, 0.0, 0.0, 1.0],
    ];
    let skin = builder.add_skin(&[joint], Some(&[inverse_bind]), Some(joint));
    builder.set_node_skin(mesh_node, skin);
    builder.add_trs_animation(
        joint,
        "translation",
        &[0.0, 1.0],
        &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        "LINEAR",
    );
    let gltf = builder.parse(&[parent]);
    let (scene, _) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
    assert_eq!(scene.skins.len(), 1);
    assert_eq!(scene.nodes[mesh_node].skin, Some(0));

    let mut options = ImportOptions::default();
    options.mv3.vertex_scale = 1.0;
    options.mv3.ticks_per_second = 100.0;
    let (mv3_file, _) = mv3::convert(&scene, &options).expect("bake skin");
    let model = &mv3_file.models[0];
    assert_eq!(model.frame_count, 2);
    assert_eq!(model.frames[0].timestamp, 0);
    assert_eq!(model.frames[1].timestamp, 100);
    assert_eq!(model.frames[0].vertices[0].x, 0);
    assert_eq!(model.frames[1].vertices[0].x, 1);
}

#[test]
fn mv3_resamples_skeletal_rotation_before_baking() {
    let positions = [[1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 1.0, 0.0]];
    let uv0 = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
    let mut builder = SceneBuilder::new();
    let mesh = builder.add_triangle_mesh(&positions, None, &uv0, &[0, 1, 2], None, &[]);
    builder.set_mesh_skin_attributes(mesh, &[[0, 0, 0, 0]; 3], &[[1.0, 0.0, 0.0, 0.0]; 3]);
    let joint = builder.add_node(None, &[], None, None, None);
    let mesh_node = builder.add_node(Some(mesh), &[], None, None, None);
    let identity = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let skin = builder.add_skin(&[joint], Some(&[identity]), Some(joint));
    builder.set_node_skin(mesh_node, skin);
    builder.add_trs_animation(
        joint,
        "rotation",
        &[0.0, 1.0],
        &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
        "LINEAR",
    );
    let gltf = builder.parse(&[joint, mesh_node]);
    let (scene, _) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");

    let mut options = ImportOptions::default();
    options.mv3.vertex_scale = 0.01;
    options.mv3.ticks_per_second = 100.0;
    let (mv3_file, _) = mv3::convert(&scene, &options).expect("bake rotation");
    let model = &mv3_file.models[0];
    let midpoint = model
        .frames
        .iter()
        .find(|frame| frame.timestamp == 50)
        .expect("30 fps resampling includes t=0.5");
    assert!(midpoint.vertices[0].x.abs() <= 1);
    assert!((midpoint.vertices[0].y - 100).abs() <= 1);
}

#[test]
fn mv3_rejects_cyclic_node_hierarchy_without_recursing() {
    let mut scene = overflow_prone_scene();
    scene.nodes[0].children = vec![1];
    let mut parent = super::ImportedNode::identity("node1");
    parent.children = vec![0];
    scene.nodes.push(parent);

    let error = mv3::convert(&scene, &ImportOptions::default()).unwrap_err();
    assert!(matches!(error, ImportError::NodeHierarchyCycle { .. }));
}

/// A parent "join" node (no mesh, identity transform) with a mesh-bearing
/// child that has both an animated translation (2 keyframes) and a
/// static (non-animated, non-identity) rotation: exercises CVD's node
/// hierarchy, the animated-keyframe path, and the single-keyframe
/// static-transform fallback path together.
#[test]
fn cvd_hierarchy_and_trs_conversion() {
    let (positions, normals, uv0, indices) = quad();
    let mut builder = SceneBuilder::new();
    let mesh = builder.add_triangle_mesh(&positions, Some(&normals), &uv0, &indices, None, &[]);
    // 90 degree rotation around Y: (0, sin(45deg), 0, cos(45deg)).
    let half = std::f32::consts::FRAC_PI_4;
    let rotation = [0.0, half.sin(), 0.0, half.cos()];
    let child = builder.add_node(Some(mesh), &[], None, Some(rotation), None);
    builder.add_trs_animation(
        child,
        "translation",
        &[0.0, 1.0],
        &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        "LINEAR",
    );
    let parent = builder.add_node(None, &[child], None, None, None);
    let gltf = builder.parse(&[parent]);

    let (scene, _diag) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
    // Sanity-check the IR captured the static rotation and the channel
    // targeting the *child*'s node index (not the parent's).
    assert_eq!(scene.nodes[child].rotation, rotation);
    assert!(
        scene
            .animations
            .iter()
            .flat_map(|a| &a.trs_channels)
            .any(|c| c.node == child && c.property == TrsProperty::Translation)
    );

    let (cvd_file, _diag) = cvd::convert(&scene, &ImportOptions::default()).expect("convert");
    assert_eq!(cvd_file.models.len(), 1);
    let root = &cvd_file.models[0];
    assert!(root.model.is_none(), "parent join node must have no model");
    assert_eq!(root.children.len(), 1);

    let child_model = root.children[0]
        .model
        .as_ref()
        .expect("child node has a mesh");

    let pos_kf = child_model
        .position_keyframes
        .as_ref()
        .expect("animated translation channel");
    assert_eq!(pos_kf.frames.len(), 2);
    assert_eq!(pos_kf.frames[0].timestamp, 0.0);
    assert_eq!(pos_kf.frames[1].timestamp, 1.0);
    // encode_position_keyframe(t, [X,Y,Z]) => unknown[1]=X, unknown[2]=-Z, unknown[3]=Y.
    assert_eq!(pos_kf.frames[1].unknown[1], 1.0);
    assert_eq!(pos_kf.frames[1].unknown[2], 0.0);
    assert_eq!(pos_kf.frames[1].unknown[3], 0.0);

    let rot_kf = child_model
        .rotation_keyframes
        .as_ref()
        .expect("static non-identity rotation");
    assert_eq!(
        rot_kf.frames.len(),
        1,
        "a static transform with no animation channel synthesizes exactly one keyframe"
    );
    // encode_rotation_keyframe(t, [x,y,z,w]) => unknown[1]=-x, unknown[2]=z, unknown[3]=-y, unknown[4]=w.
    let frame = &rot_kf.frames[0];
    assert!((frame.unknown[1] - (-rotation[0])).abs() < 1e-5);
    assert!((frame.unknown[2] - rotation[2]).abs() < 1e-5);
    assert!((frame.unknown[3] - (-rotation[1])).abs() < 1e-5);
    assert!((frame.unknown[4] - rotation[3]).abs() < 1e-5);

    // Round-trip through the real (non-seekable) fileformats reader/writer.
    let mut bytes = Vec::new();
    fileformats::pal3::cvd::write_cvd(&mut bytes, &cvd_file).expect("write");
    let read_back = fileformats::pal3::cvd::read_cvd(&mut Cursor::new(&bytes)).expect("read back");
    assert_eq!(read_back.models.len(), 1);
    assert!(read_back.models[0].model.is_none());
    assert_eq!(read_back.models[0].children.len(), 1);
    assert_eq!(
        read_back.models[0].children[0]
            .model
            .as_ref()
            .unwrap()
            .position_keyframes
            .as_ref()
            .unwrap()
            .frames
            .len(),
        2
    );
}

/// High-level [`convert_gltf_to_bytes`] with a replacement-template
/// fallback: a plain (no `asset.extras.yaobow`) glTF has no way to
/// recover an action table or texture reserved fields, so those should
/// come from an existing `.mv3` template file's opaque metadata instead.
#[test]
fn convert_gltf_to_bytes_falls_back_to_replacement_template() {
    let (positions, normals, uv0, indices) = quad();
    let mut builder = SceneBuilder::new();
    let mesh = builder.add_triangle_mesh(&positions, Some(&normals), &uv0, &indices, None, &[]);
    let node = builder.add_node(Some(mesh), &[], None, None, None);
    let gltf = builder.parse(&[node]);
    let bytes = gltf_to_glb_bytes(&gltf);

    let scratch_dir = scratch_dir();
    std::fs::create_dir_all(&scratch_dir).expect("create scratch dir");
    let scratch_path = scratch_dir.join("convert_gltf_to_bytes_template_fallback.glb");
    std::fs::write(&scratch_path, &bytes).expect("write scratch glb");

    let template = Mv3File {
        version: 7,
        duration: 0,
        texture_count: 1,
        unknown_data_count: 0,
        model_count: 0,
        action_count: 1,
        action_desc: vec![Mv3ActionDesc {
            tick: 99,
            name: super::fixed_capacity_string("run", 16),
        }],
        unknown_data: vec![],
        textures: vec![Mv3Texture {
            unknown: vec![0.0; 17],
            names: vec![
                SizedString::from("template_tex.bmp"),
                SizedString::from(""),
                SizedString::from(""),
                SizedString::from(""),
            ],
        }],
        models: vec![],
    };
    let mut template_bytes = Vec::new();
    fileformats::mv3::write_mv3(&mut Cursor::new(&mut template_bytes), &template)
        .expect("write template");

    let result = convert_gltf_to_bytes(
        &scratch_path,
        TargetFormat::Mv3,
        &ImportOptions::default(),
        Some(&template_bytes),
    );
    let _ = std::fs::remove_file(&scratch_path);
    let (out_bytes, diagnostics) = result.expect("convert_gltf_to_bytes");

    assert!(
        diagnostics
            .messages()
            .any(|m| m.contains("falling back to the replacement template")),
        "expected a template-fallback diagnostic, got: {:?}",
        diagnostics
    );

    let read_back = fileformats::mv3::read_mv3(&mut Cursor::new(&out_bytes)).expect("read back");
    assert_eq!(read_back.action_desc.len(), 1);
    assert_eq!(read_back.action_desc[0].name.as_str().unwrap(), "run");
    assert_eq!(
        read_back.textures[0].names[0].to_string().unwrap(),
        "template_tex.bmp"
    );
}

/// Full export -> import round trip for a POL mesh whose **first**
/// material has zero triangles and whose **second** material is the
/// only one with real geometry: [`export_pol_to_glb`] must skip the
/// empty material when building glTF primitives *and* filter
/// `asset.extras.yaobow.payload.material_metadata` the same way, so the
/// surviving primitive's metadata (texture name + reserved marker
/// fields) still lines up positionally after
/// [`super::pol::convert`] reads it back. Before the fix, the unfiltered
/// metadata list kept the empty material's (all-default) entry at index
/// 0, silently attaching it to the one real primitive instead of the
/// non-empty material's own metadata.
#[test]
fn pol_material_metadata_survives_leading_empty_material() {
    use fileformats::pol::{
        GeomNodeDesc, PolFile, PolMaterialInfo, PolMesh, PolTriangle, PolVertex,
        PolVertexComponents,
    };
    use fileformats::rwbs::Vec3f;

    let vertex_components = {
        use fileformats::binrw::BinRead;
        // POSITION (0b1) | TEXCOORD (0b10000).
        let bits: u32 = 0b1 | 0b10000;
        PolVertexComponents::read(&mut Cursor::new(bits.to_le_bytes())).unwrap()
    };

    let vertex = |x: f32, y: f32, u: f32, v: f32| PolVertex {
        position: Vec3f { x, y, z: 0.0 },
        normal: None,
        unknown4: None,
        unknown8: None,
        tex_coord: TexCoord { u, v },
        tex_coord2: None,
        unknown40: None,
        unknown80: None,
        unknown100: None,
    };
    let vertices = vec![
        vertex(0.0, 0.0, 0.0, 0.0),
        vertex(1.0, 0.0, 1.0, 0.0),
        vertex(0.0, 1.0, 0.0, 1.0),
    ];

    // First material: no triangles at all -> the exporter must skip it
    // entirely (no `Primitive` emitted for it).
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
    // Second material: real geometry, with distinctive marker values we
    // can check for after the round trip.
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
        aabb_min: Vec3f {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        aabb_max: Vec3f {
            x: 1.0,
            y: 1.0,
            z: 0.0,
        },
        vertex_type: vertex_components,
        vertex_count: 3,
        vertices,
        material_info_count: 2,
        material_info: vec![empty_material, real_material],
    };
    let pol = PolFile {
        some_flag: 0,
        mesh_count: 1,
        geom_node_descs: vec![GeomNodeDesc {
            unknown: vec![0u16; 26],
        }],
        unknown_count: 0,
        unknown_data: vec![],
        meshes: vec![mesh],
    };

    let vfs = mini_fs::MiniFs::new(false);
    let glb_bytes = crate::exporters::gltf::export_pol_to_glb(&pol, &vfs, Path::new("/d/x.pol"))
        .expect("export_pol_to_glb succeeds");

    let gltf = gltf::Gltf::from_slice(&glb_bytes).expect("re-parsing exported glb");
    let (scene, diagnostics) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        diagnostics
    );

    let (pol_file, convert_diag) =
        pol::convert(&scene, &ImportOptions::default()).expect("convert");
    assert!(
        convert_diag.is_empty(),
        "unexpected conversion diagnostics: {:?}",
        convert_diag
    );

    assert_eq!(pol_file.meshes.len(), 1);
    let mesh = &pol_file.meshes[0];
    // Only the non-empty material survives as a primitive/material_info.
    assert_eq!(mesh.material_info.len(), 1);
    let mat = &mesh.material_info[0];
    assert_eq!(mat.texture_names[0].as_str().unwrap(), "second.bmp");
    assert_eq!(mat.unknown2, 42);
    assert_eq!(mat.unknown3, 43);
    assert_eq!(mat.unknown4, 44);
    assert_eq!(mat.unknown_68, vec![9.5; 16]);
}

/// Full export -> import round trip for a CVD mesh whose **first**
/// material has zero triangles (`triangles: None`) and whose **second**
/// material is the only one with real geometry. Mirrors the POL test
/// above but for [`export_cvd_to_glb`] / [`super::cvd::convert`]: the
/// importer must filter `extras.mesh.materials` by `triangle_count > 0`
/// before indexing it positionally by primitive index, or the surviving
/// primitive would incorrectly inherit the empty material's (default)
/// texture name and colors.
#[test]
fn cvd_material_metadata_survives_leading_empty_material() {
    use crate::exporters::gltf::export_cvd_to_glb;
    use crate::openpal3::loaders::cvd_loader::{
        CvdFile, CvdMaterial, CvdMesh, CvdModel, CvdModelNode, CvdTriangle, CvdVertex,
    };
    use radiance::math::{Vec2, Vec3};

    let vertex = |x: f32, y: f32| CvdVertex {
        position: Vec3 { x, y, z: 0.0 },
        normal: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        tex_coord: Vec2 { x, y },
    };
    let frame = vec![vertex(0.0, 0.0), vertex(1.0, 0.0), vertex(0.0, 1.0)];

    let empty_material = CvdMaterial {
        unknown_byte: 0,
        color1: 0,
        color2: 0,
        color3: 0,
        color4: 0,
        texture_name: String::new(),
        triangle_count: 0,
        triangles: None,
    };
    let real_material = CvdMaterial {
        unknown_byte: 5,
        color1: 0x11111111,
        color2: 0x22222222,
        color3: 0x33333333,
        color4: 0x44444444,
        texture_name: "cvd_second.bmp".to_string(),
        triangle_count: 1,
        triangles: Some(vec![CvdTriangle { indices: [0, 1, 2] }]),
    };
    let mesh = CvdMesh {
        frame_count: 1,
        vertex_count: 3,
        frames: vec![frame],
        unknown_data: vec![0.0],
        material_count: 2,
        materials: vec![empty_material, real_material],
    };
    let model = CvdModel {
        unknown_byte: 0,
        scale_factor: 1.0,
        position_keyframes: None,
        rotation_keyframes: None,
        scale_keyframes: None,
        mesh,
    };
    let cvd = CvdFile {
        magic: *b"cvdf",
        model_count: 1,
        models: vec![CvdModelNode {
            model: Some(model),
            children: None,
        }],
    };

    let vfs = mini_fs::MiniFs::new(false);
    let glb_bytes =
        export_cvd_to_glb(&cvd, &vfs, Path::new("/d/x.cvd")).expect("export_cvd_to_glb succeeds");

    let gltf = gltf::Gltf::from_slice(&glb_bytes).expect("re-parsing exported glb");
    let (scene, diagnostics) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        diagnostics
    );

    let (cvd_file, convert_diag) =
        cvd::convert(&scene, &ImportOptions::default()).expect("convert");
    assert!(
        convert_diag.is_empty(),
        "unexpected conversion diagnostics: {:?}",
        convert_diag
    );

    assert_eq!(cvd_file.models.len(), 1);
    let model = cvd_file.models[0].model.as_ref().expect("model has a mesh");
    // Only the non-empty material survives as a primitive/CvdMaterial.
    assert_eq!(model.mesh.materials.len(), 1);
    let mat = &model.mesh.materials[0];
    assert_eq!(mat.texture_name, "cvd_second.bmp");
    assert_eq!(mat.color1, 0x11111111);
    assert_eq!(mat.color2, 0x22222222);
    assert_eq!(mat.color3, 0x33333333);
    assert_eq!(mat.color4, 0x44444444);
    assert_eq!(mat.unknown_byte, 5);
}

/// Full export -> import round trip for a POL mesh with **two** real
/// (non-empty) materials that both reference the *same* shared vertex
/// buffer — the shape every Yaobow-exported multi-material POL mesh
/// actually has (see `exporters::gltf::pol`: one shared position/UV
/// accessor per node, one glTF `Primitive` per material). Before the
/// `importers::pol::build_mesh` vertex-pooling fix, each primitive's full
/// (shared) vertex array got appended into the pool again per material,
/// so this 6-vertex/2-material mesh would have round-tripped back out as
/// 12 vertices instead of 6.
#[test]
fn pol_round_trip_preserves_vertex_count_across_shared_material_buffer() {
    use fileformats::pol::{
        GeomNodeDesc, PolFile, PolMaterialInfo, PolMesh, PolTriangle, PolVertex,
        PolVertexComponents,
    };
    use fileformats::rwbs::Vec3f;

    let vertex_components = {
        use fileformats::binrw::BinRead;
        // POSITION (0b1) | TEXCOORD (0b10000).
        let bits: u32 = 0b1 | 0b10000;
        PolVertexComponents::read(&mut Cursor::new(bits.to_le_bytes())).unwrap()
    };

    let vertex = |x: f32, y: f32, u: f32, v: f32| PolVertex {
        position: Vec3f { x, y, z: 0.0 },
        normal: None,
        unknown4: None,
        unknown8: None,
        tex_coord: TexCoord { u, v },
        tex_coord2: None,
        unknown40: None,
        unknown80: None,
        unknown100: None,
    };
    // 6 unique vertices shared by both materials: material A only uses
    // 0..3, material B only uses 3..6 - exactly like a real
    // multi-material POL mesh (one shared vertex pool, disjoint triangle
    // groups per material).
    let vertices = vec![
        vertex(0.0, 0.0, 0.0, 0.0),
        vertex(1.0, 0.0, 1.0, 0.0),
        vertex(0.0, 1.0, 0.0, 1.0),
        vertex(2.0, 0.0, 0.0, 0.0),
        vertex(3.0, 0.0, 1.0, 0.0),
        vertex(2.0, 1.0, 0.0, 1.0),
    ];

    let material_a = PolMaterialInfo {
        use_alpha: 0,
        unknown_68: vec![0.0; 16],
        unknown_float: 0.0,
        texture_count: 1,
        texture_names: vec!["first.bmp".into()],
        unknown2: 0,
        unknown3: 0,
        unknown4: 0,
        triangle_count: 1,
        triangles: vec![PolTriangle { indices: [0, 1, 2] }],
    };
    let material_b = PolMaterialInfo {
        use_alpha: 1,
        unknown_68: vec![1.0; 16],
        unknown_float: 5.0,
        texture_count: 1,
        texture_names: vec!["second.bmp".into()],
        unknown2: 1,
        unknown3: 2,
        unknown4: 3,
        triangle_count: 1,
        triangles: vec![PolTriangle { indices: [3, 4, 5] }],
    };
    let mesh = PolMesh {
        aabb_min: Vec3f {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        aabb_max: Vec3f {
            x: 3.0,
            y: 1.0,
            z: 0.0,
        },
        vertex_type: vertex_components,
        vertex_count: 6,
        vertices,
        material_info_count: 2,
        material_info: vec![material_a, material_b],
    };
    let pol = PolFile {
        some_flag: 0,
        mesh_count: 1,
        geom_node_descs: vec![GeomNodeDesc {
            unknown: vec![0u16; 26],
        }],
        unknown_count: 0,
        unknown_data: vec![],
        meshes: vec![mesh],
    };

    let vfs = mini_fs::MiniFs::new(false);
    let glb_bytes = crate::exporters::gltf::export_pol_to_glb(&pol, &vfs, Path::new("/d/x.pol"))
        .expect("export_pol_to_glb succeeds");

    let gltf = gltf::Gltf::from_slice(&glb_bytes).expect("re-parsing exported glb");
    let (scene, diagnostics) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        diagnostics
    );

    // Confirm the synthetic setup actually reproduces the shared-buffer
    // shape the exporter produces: both primitives carry the full
    // 6-vertex array.
    let node = scene
        .nodes
        .iter()
        .find(|n| n.mesh.is_some())
        .expect("mesh node");
    let mesh_in = node.mesh.as_ref().unwrap();
    assert_eq!(mesh_in.primitives.len(), 2);
    assert_eq!(mesh_in.primitives[0].positions.len(), 6);
    assert_eq!(mesh_in.primitives[1].positions.len(), 6);

    let (pol_file, convert_diag) =
        pol::convert(&scene, &ImportOptions::default()).expect("convert");
    assert!(
        convert_diag.is_empty(),
        "unexpected conversion diagnostics: {:?}",
        convert_diag
    );

    assert_eq!(pol_file.meshes.len(), 1);
    let mesh_out = &pol_file.meshes[0];
    assert_eq!(
        mesh_out.vertex_count, 6,
        "the shared vertex buffer must be pooled once, not duplicated per material"
    );
    assert_eq!(mesh_out.vertices.len(), 6);
    assert_eq!(mesh_out.material_info.len(), 2);
    assert_eq!(
        mesh_out.material_info[0].texture_names[0].as_str().unwrap(),
        "first.bmp"
    );
    assert_eq!(mesh_out.material_info[0].triangles.len(), 1);
    assert_eq!(mesh_out.material_info[0].triangles[0].indices, [0, 1, 2]);
    assert_eq!(
        mesh_out.material_info[1].texture_names[0].as_str().unwrap(),
        "second.bmp"
    );
    assert_eq!(mesh_out.material_info[1].triangles.len(), 1);
    assert_eq!(mesh_out.material_info[1].triangles[0].indices, [3, 4, 5]);

    let mut bytes = Vec::new();
    pol::write(
        &scene,
        &ImportOptions::default(),
        &mut Cursor::new(&mut bytes),
    )
    .expect("write");
    let read_back = fileformats::pol::read_pol(&mut Cursor::new(&bytes)).expect("read back");
    assert_eq!(read_back.meshes.len(), 1);
    assert_eq!(read_back.meshes[0].vertex_count, 6);
    assert_eq!(read_back.meshes[0].material_info.len(), 2);
    assert_eq!(
        read_back.meshes[0].material_info[0].triangles[0].indices,
        [0, 1, 2]
    );
    assert_eq!(
        read_back.meshes[0].material_info[1].triangles[0].indices,
        [3, 4, 5]
    );
}

/// Full export -> import round trip for a CVD mesh with **two** real
/// (non-empty) materials that both reference the *same* shared
/// per-frame vertex buffer — mirrors the POL test above, but for
/// [`export_cvd_to_glb`] / [`super::cvd::convert`] (see
/// `exporters::gltf::cvd`: one shared position/normal/UV accessor per
/// node, one glTF `Primitive` per material).
#[test]
fn cvd_round_trip_preserves_vertex_count_across_shared_material_buffer() {
    use crate::exporters::gltf::export_cvd_to_glb;
    use crate::openpal3::loaders::cvd_loader::{
        CvdFile, CvdMaterial, CvdMesh, CvdModel, CvdModelNode, CvdTriangle, CvdVertex,
    };
    use radiance::math::{Vec2, Vec3};

    let vertex = |x: f32, y: f32| CvdVertex {
        position: Vec3 { x, y, z: 0.0 },
        normal: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        tex_coord: Vec2 { x, y },
    };
    // 6 unique vertices shared by both materials: material A only uses
    // 0..3, material B only uses 3..6 (same shared-buffer shape as the
    // POL test above).
    let frame = vec![
        vertex(0.0, 0.0),
        vertex(1.0, 0.0),
        vertex(0.0, 1.0),
        vertex(2.0, 0.0),
        vertex(3.0, 0.0),
        vertex(2.0, 1.0),
    ];

    let material_a = CvdMaterial {
        unknown_byte: 0,
        color1: 0x11111111,
        color2: 0x22222222,
        color3: 0x33333333,
        color4: 0x44444444,
        texture_name: "cvd_first.bmp".to_string(),
        triangle_count: 1,
        triangles: Some(vec![CvdTriangle { indices: [0, 1, 2] }]),
    };
    let material_b = CvdMaterial {
        unknown_byte: 5,
        color1: 0x55555555,
        color2: 0x66666666,
        color3: 0x77777777,
        color4: 0x88888888,
        texture_name: "cvd_second.bmp".to_string(),
        triangle_count: 1,
        triangles: Some(vec![CvdTriangle { indices: [3, 4, 5] }]),
    };
    let mesh = CvdMesh {
        frame_count: 1,
        vertex_count: 6,
        frames: vec![frame],
        unknown_data: vec![0.0],
        material_count: 2,
        materials: vec![material_a, material_b],
    };
    let model = CvdModel {
        unknown_byte: 0,
        scale_factor: 1.0,
        position_keyframes: None,
        rotation_keyframes: None,
        scale_keyframes: None,
        mesh,
    };
    let cvd = CvdFile {
        magic: *b"cvdf",
        model_count: 1,
        models: vec![CvdModelNode {
            model: Some(model),
            children: None,
        }],
    };

    let vfs = mini_fs::MiniFs::new(false);
    let glb_bytes =
        export_cvd_to_glb(&cvd, &vfs, Path::new("/d/x.cvd")).expect("export_cvd_to_glb succeeds");

    let gltf = gltf::Gltf::from_slice(&glb_bytes).expect("re-parsing exported glb");
    let (scene, diagnostics) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        diagnostics
    );

    let node = scene
        .nodes
        .iter()
        .find(|n| n.mesh.is_some())
        .expect("mesh node");
    let mesh_in = node.mesh.as_ref().unwrap();
    assert_eq!(mesh_in.primitives.len(), 2);
    assert_eq!(mesh_in.primitives[0].positions.len(), 6);
    assert_eq!(mesh_in.primitives[1].positions.len(), 6);

    let (cvd_file, convert_diag) =
        cvd::convert(&scene, &ImportOptions::default()).expect("convert");
    assert!(
        convert_diag.is_empty(),
        "unexpected conversion diagnostics: {:?}",
        convert_diag
    );

    assert_eq!(cvd_file.models.len(), 1);
    let model = cvd_file.models[0].model.as_ref().expect("model has a mesh");
    assert_eq!(
        model.mesh.vertex_count, 6,
        "the shared vertex buffer must be pooled once, not duplicated per material"
    );
    assert_eq!(model.mesh.frames[0].len(), 6);
    assert_eq!(model.mesh.materials.len(), 2);
    assert_eq!(model.mesh.materials[0].texture_name, "cvd_first.bmp");
    assert_eq!(model.mesh.materials[0].triangles[0].indices, [0, 1, 2]);
    assert_eq!(model.mesh.materials[1].texture_name, "cvd_second.bmp");
    assert_eq!(model.mesh.materials[1].triangles[0].indices, [3, 4, 5]);

    let mut bytes = Vec::new();
    cvd::write(&scene, &ImportOptions::default(), &mut bytes).expect("write");
    let read_back = fileformats::pal3::cvd::read_cvd(&mut Cursor::new(&bytes)).expect("read back");
    assert_eq!(read_back.models.len(), 1);
    let model = read_back.models[0].model.as_ref().expect("model");
    assert_eq!(model.mesh.vertex_count, 6);
    assert_eq!(model.mesh.materials.len(), 2);
    assert_eq!(model.mesh.materials[0].triangles[0].indices, [0, 1, 2]);
    assert_eq!(model.mesh.materials[1].triangles[0].indices, [3, 4, 5]);
}

/// MV3 texture/action names are read back with GBK decoding (see
/// `fileformats::utils::SizedString::to_string` /
/// `StringWithCapacity::as_str`), so the importer must GBK-encode them
/// on write too. A Chinese texture name (via `asset.extras.yaobow`'s
/// `textures[].names`) and a Chinese action name (`action_desc[].name`)
/// must both survive an `mv3::convert` + on-disk-format write/read round
/// trip unchanged.
#[test]
fn mv3_chinese_texture_and_action_names_round_trip() {
    let (positions, normals, uv0, indices) = quad();
    let mut builder = SceneBuilder::new();
    let mesh = builder.add_triangle_mesh(&positions, Some(&normals), &uv0, &indices, None, &[]);
    let node = builder.add_node(Some(mesh), &[], None, None, None);
    builder.set_asset_extras(serde_json::json!({
        "yaobow": {
            "schema": 1,
            "payload": {
                "target_format": "mv3",
                "version": 4,
                "action_desc": [{ "tick": 10, "name": "行走" }],
                "textures": [{
                    "unknown": vec![0.0f32; 17],
                    "names": ["纹理.bmp", "", "", ""],
                }],
                "file_unknown_data": [],
                "model_metadata": [],
            }
        }
    }));
    let gltf = builder.parse(&[node]);

    let (scene, diagnostics) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        diagnostics
    );

    let (mv3_file, convert_diag) =
        mv3::convert(&scene, &ImportOptions::default()).expect("convert");
    assert!(
        convert_diag.is_empty(),
        "unexpected conversion diagnostics: {:?}",
        convert_diag
    );
    assert_eq!(mv3_file.action_desc.len(), 1);
    assert_eq!(mv3_file.action_desc[0].name.as_str().unwrap(), "行走");
    assert_eq!(
        mv3_file.textures[0].names[0].to_string().unwrap(),
        "纹理.bmp"
    );

    let mut bytes = Vec::new();
    fileformats::mv3::write_mv3(&mut Cursor::new(&mut bytes), &mv3_file).expect("write");
    let read_back = fileformats::mv3::read_mv3(&mut Cursor::new(&bytes)).expect("read back");
    assert_eq!(read_back.action_desc[0].name.as_str().unwrap(), "行走");
    assert_eq!(
        read_back.textures[0].names[0].to_string().unwrap(),
        "纹理.bmp"
    );
}

/// An MV3 action name that GBK-encodes to more than 16 bytes must be a
/// hard [`ImportError::NameTooLong`], not a silently truncated/corrupted
/// value. Asserting `actual == 18` (not `27`, the name's UTF-8 byte
/// length) confirms the capacity check itself measures the GBK
/// encoding, not `str::len()`.
#[test]
fn mv3_action_name_too_long_after_gbk_encoding_is_an_error() {
    let (positions, normals, uv0, indices) = quad();
    let mut builder = SceneBuilder::new();
    let mesh = builder.add_triangle_mesh(&positions, Some(&normals), &uv0, &indices, None, &[]);
    let node = builder.add_node(Some(mesh), &[], None, None, None);
    // 9 Chinese characters = 18 GBK bytes (2 bytes/char, > the 16-byte
    // capacity), but 27 UTF-8 bytes (3 bytes/char).
    let long_name = "一二三四五六七八九";
    builder.set_asset_extras(serde_json::json!({
        "yaobow": {
            "schema": 1,
            "payload": {
                "target_format": "mv3",
                "version": 4,
                "action_desc": [{ "tick": 10, "name": long_name }],
                "textures": [],
                "file_unknown_data": [],
                "model_metadata": [],
            }
        }
    }));
    let gltf = builder.parse(&[node]);

    let (scene, _diagnostics) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
    let err = mv3::convert(&scene, &ImportOptions::default())
        .expect_err("GBK-encoded action name exceeding 16 bytes must fail");
    assert!(
        matches!(
            err,
            ImportError::NameTooLong {
                actual: 18,
                limit: 16,
                ..
            }
        ),
        "unexpected error: {:?}",
        err
    );
}

/// An 8-character Chinese action name GBK-encodes to exactly 16 bytes
/// (2 bytes/char) — it fits the capacity exactly and must be accepted
/// and round-trip correctly, even though its UTF-8 encoding is 24 bytes
/// (which a UTF-8-length-based capacity check would have wrongly
/// rejected as too long).
#[test]
fn mv3_action_name_exactly_fills_gbk_capacity() {
    let (positions, normals, uv0, indices) = quad();
    let mut builder = SceneBuilder::new();
    let mesh = builder.add_triangle_mesh(&positions, Some(&normals), &uv0, &indices, None, &[]);
    let node = builder.add_node(Some(mesh), &[], None, None, None);
    let exact_name = "一二三四五六七八"; // 8 chars: 16 GBK bytes, 24 UTF-8 bytes.
    builder.set_asset_extras(serde_json::json!({
        "yaobow": {
            "schema": 1,
            "payload": {
                "target_format": "mv3",
                "version": 4,
                "action_desc": [{ "tick": 3, "name": exact_name }],
                "textures": [],
                "file_unknown_data": [],
                "model_metadata": [],
            }
        }
    }));
    let gltf = builder.parse(&[node]);

    let (scene, _diagnostics) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
    let (mv3_file, _diag) = mv3::convert(&scene, &ImportOptions::default())
        .expect("an exactly-16-GBK-byte name must fit");
    assert_eq!(mv3_file.action_desc[0].name.as_str().unwrap(), exact_name);

    let mut bytes = Vec::new();
    fileformats::mv3::write_mv3(&mut Cursor::new(&mut bytes), &mv3_file).expect("write");
    let read_back = fileformats::mv3::read_mv3(&mut Cursor::new(&bytes)).expect("read back");
    assert_eq!(read_back.action_desc[0].name.as_str().unwrap(), exact_name);
}

/// POL texture names are also read back with GBK decoding
/// (`StringWithCapacity::as_str`); a Chinese texture name sourced from
/// the glTF material's base-color image URI must survive an
/// `pol::convert` + on-disk-format write/read round trip unchanged.
#[test]
fn pol_chinese_texture_name_round_trips_via_gbk() {
    let dir = scratch_named_dir("pol-chinese-texture");
    std::fs::write(dir.join("纹理.bmp"), one_pixel_png()).unwrap();
    let (positions, normals, uv0, indices) = quad();
    let mut builder = SceneBuilder::new();
    let image = builder.add_image_uri("纹理.bmp");
    let texture = builder.add_texture(image);
    let material = builder.add_material(Some(texture), false);
    let mesh = builder.add_triangle_mesh(
        &positions,
        Some(&normals),
        &uv0,
        &indices,
        Some(material),
        &[],
    );
    let node = builder.add_node(Some(mesh), &[], None, None, None);
    let gltf = builder.parse(&[node]);

    let (scene, diagnostics) = load_gltf_scene_from(&gltf, &dir).expect("load");
    assert!(
        diagnostics.messages().any(|m| m.contains("纹理.tga")),
        "expected texture conversion diagnostic: {:?}",
        diagnostics
    );

    let (pol_file, convert_diag) =
        pol::convert(&scene, &ImportOptions::default()).expect("convert");
    assert!(
        convert_diag.is_empty(),
        "unexpected conversion diagnostics: {:?}",
        convert_diag
    );
    assert_eq!(
        pol_file.meshes[0].material_info[0].texture_names[0]
            .as_str()
            .unwrap(),
        "_yaobow_import/纹理.tga"
    );

    let mut bytes = Vec::new();
    pol::write(
        &scene,
        &ImportOptions::default(),
        &mut Cursor::new(&mut bytes),
    )
    .expect("write");
    let read_back = fileformats::pol::read_pol(&mut Cursor::new(&bytes)).expect("read back");
    assert_eq!(
        read_back.meshes[0].material_info[0].texture_names[0]
            .as_str()
            .unwrap(),
        "_yaobow_import/纹理.tga"
    );
}

/// GBK round trip for `.pol`'s `ddd_str` (an opaque per-`UnknownData`
/// string, only ever emitted when the source/template `some_flag > 100`)
/// via `asset.extras.yaobow` metadata: this exercises
/// `PolExtrasUnknownData::try_into_unknown_data`'s GBK encoding, which
/// replaced a raw-UTF-8 `SizedString::from` that silently corrupted any
/// non-ASCII `ddd_str` on POL's GBK-decoding read path.
#[test]
fn pol_ddd_str_chinese_round_trips_via_gbk() {
    let (positions, normals, uv0, indices) = quad();
    let mut builder = SceneBuilder::new();
    let mesh = builder.add_triangle_mesh(&positions, Some(&normals), &uv0, &indices, None, &[]);
    let node = builder.add_node(Some(mesh), &[], None, None, None);
    let matrix = vec![0.0f32; 16];
    // `UnknownData::unknown` is a fixed 32-byte field on read
    // (`#[br(count = 32)]` in `fileformats::pol::UnknownData`); supply
    // exactly 32 bytes here so the write/read round trip below doesn't
    // desync the following `matrix`/`unknown2`/`ddd_str` fields.
    let unknown = vec![0u8; 32];
    builder.set_asset_extras(serde_json::json!({
        "yaobow": {
            "schema": 1,
            "payload": {
                "target_format": "pol",
                "some_flag": 101,
                "geom_node_descs": [],
                "unknown_data": [{
                    "unknown": unknown,
                    "matrix": matrix,
                    "unknown2": 0,
                    "ddd_str": "中文测试字符串",
                }],
                "material_metadata": [],
            }
        }
    }));
    let gltf = builder.parse(&[node]);

    let (scene, diagnostics) = load_gltf_scene_from(&gltf, Path::new(".")).expect("load");
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        diagnostics
    );

    let (pol_file, convert_diag) =
        pol::convert(&scene, &ImportOptions::default()).expect("convert");
    assert!(
        convert_diag.is_empty(),
        "unexpected conversion diagnostics: {:?}",
        convert_diag
    );
    assert_eq!(pol_file.unknown_data.len(), 1);
    assert_eq!(
        pol_file.unknown_data[0].ddd_str.to_string().unwrap(),
        "中文测试字符串"
    );

    let mut bytes = Vec::new();
    pol::write(
        &scene,
        &ImportOptions::default(),
        &mut Cursor::new(&mut bytes),
    )
    .expect("write");
    let read_back = fileformats::pol::read_pol(&mut Cursor::new(&bytes)).expect("read back");
    assert_eq!(
        read_back.unknown_data[0].ddd_str.to_string().unwrap(),
        "中文测试字符串"
    );
}

/// A malformed/adversarial [`super::ImportedPrimitive`] whose second
/// primitive's raw index is `u32::MAX` — combined with even a tiny
/// nonzero pooling `base_index` (from a single earlier primitive), the
/// remap addition (`raw + base_index`) overflows `u32`. Real glTF input
/// can never reach this through [`super::load_gltf_scene`] (indices are
/// u16-accessor-backed, capped at 65535, and the loader's own
/// `PrimitiveIndexOutOfBounds` check additionally rejects any index
/// beyond the primitive's own vertex count), so this constructs the
/// intermediate [`super::ImportedScene`] IR directly to exercise the
/// converters' `checked_add`/[`ImportError::IndexRemapOverflow`] guard
/// in isolation.
fn overflow_prone_scene() -> super::ImportedScene {
    let prim_a = super::ImportedPrimitive {
        positions: vec![[0.0, 0.0, 0.0]],
        normals: vec![[0.0, 1.0, 0.0]],
        uv0: vec![[0.0, 0.0]],
        indices: vec![0, 0, 0],
        material_texture: None,
        material_alpha_blend: false,
        morph_targets: vec![],
        skin_influences: None,
    };
    let prim_b = super::ImportedPrimitive {
        // Deliberately distinct vertex data from `prim_a` so pol/cvd's
        // `shares_vertex_data`-based pooling reuse doesn't collapse the
        // two primitives back into a single (zero) base index.
        positions: vec![[1.0, 1.0, 1.0]],
        normals: vec![[0.0, 1.0, 0.0]],
        uv0: vec![[1.0, 1.0]],
        indices: vec![u32::MAX, u32::MAX, u32::MAX],
        material_texture: None,
        material_alpha_blend: false,
        morph_targets: vec![],
        skin_influences: None,
    };
    let mesh = super::ImportedMesh {
        name: "mesh0".to_string(),
        primitives: vec![prim_a, prim_b],
    };
    let mut node = super::ImportedNode::identity("node0");
    node.mesh = Some(mesh);
    super::ImportedScene {
        nodes: vec![node],
        roots: vec![0],
        animations: vec![],
        skins: vec![],
        extras: None,
        textures: vec![],
    }
}

#[test]
fn pol_index_remap_overflow_is_an_error() {
    let scene = overflow_prone_scene();
    let err = pol::convert(&scene, &ImportOptions::default()).unwrap_err();
    assert!(
        matches!(err, ImportError::IndexRemapOverflow { .. }),
        "expected IndexRemapOverflow, got {err:?}"
    );
}

#[test]
fn cvd_index_remap_overflow_is_an_error() {
    let scene = overflow_prone_scene();
    let err = cvd::convert(&scene, &ImportOptions::default()).unwrap_err();
    assert!(
        matches!(err, ImportError::IndexRemapOverflow { .. }),
        "expected IndexRemapOverflow, got {err:?}"
    );
}

#[test]
fn mv3_index_remap_overflow_is_an_error() {
    let scene = overflow_prone_scene();
    let err = mv3::convert(&scene, &ImportOptions::default()).unwrap_err();
    assert!(
        matches!(err, ImportError::IndexRemapOverflow { .. }),
        "expected IndexRemapOverflow, got {err:?}"
    );
}

/// Serializes an already-parsed [`gltf::Gltf`] back into `.glb` bytes, for
/// the one test that needs an on-disk file (to exercise
/// [`convert_gltf_to_bytes`]'s file-based entry point).
fn gltf_to_glb_bytes(gltf: &gltf::Gltf) -> Vec<u8> {
    let json = gltf.document.as_json();
    let json_bytes = serde_json::to_vec(json).expect("serialize gltf json");
    let glb = gltf::binary::Glb {
        header: gltf::binary::Header {
            magic: *b"glTF",
            version: 2,
            length: 0,
        },
        json: std::borrow::Cow::Owned(json_bytes),
        bin: gltf.blob.clone().map(std::borrow::Cow::Owned),
    };
    glb.to_vec().expect("assemble glb")
}

/// A scratch directory under the workspace's (gitignored) `target/`
/// directory — never `/tmp` — for the one test that needs a real file on
/// disk.
fn scratch_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/importer_test_scratch")
}
