use byteorder::{LittleEndian, ReadBytesExt};
use fileformats::pal3::cvd as raw;
use mini_fs::{MiniFs, StoreExt};
use radiance::math::{Quaternion, Vec2, Vec3};
use serde::Serialize;
use std::error::Error;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

// Byte-level parsing (counts, endianness, string decoding, ...) lives in
// `fileformats::pal3::cvd`, which stores fields exactly as encoded on disk.
// This module owns *only* the engine-facing interpretation of that raw
// data: coordinate axis swaps, quaternion handedness fixes, and which of
// the many ambiguous per-keyframe floats represent which quantity for a
// given keyframe "version". All of that interpretation logic below is
// unchanged from the pre-refactor implementation.

#[derive(Debug, Serialize)]
pub struct CvdVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub tex_coord: Vec2,
}

#[derive(Debug, Serialize)]
pub struct CvdTriangle {
    pub indices: [u16; 3],
}

#[derive(Debug, Serialize)]
pub struct CvdMaterial {
    pub unknown_byte: u8,
    pub color1: u32,
    pub color2: u32,
    pub color3: u32,
    pub color4: u32,
    pub texture_name: String,
    pub triangle_count: u32,
    pub triangles: Option<Vec<CvdTriangle>>,
}

#[derive(Debug, Serialize)]
pub struct CvdMesh {
    pub frame_count: u32,
    pub vertex_count: u32,
    pub frames: Vec<Vec<CvdVertex>>,
    pub unknown_data: Vec<f32>,
    pub material_count: u32,
    pub materials: Vec<CvdMaterial>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdPositionKeyFrame {
    pub timestamp: f32,
    pub position: Vec3,
    pub unknown1: f32,
    pub unknown2: f32,
    pub unknown3: f32,
    pub unknown4: f32,
    pub unknown5: f32,
    pub unknown6: f32,
    pub unknown7: f32,
    pub unknown8: f32,
    pub unknown9: f32,
    pub unknown10: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdPositionKeyFrames {
    pub version: u8,
    pub frames: Vec<CvdPositionKeyFrame>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdRotationKeyFrame {
    pub timestamp: f32,
    pub quaternion: Quaternion,
    pub unknown1: f32,
    pub unknown2: f32,
    pub unknown3: f32,
    pub unknown4: f32,
    pub unknown5: f32,
    pub unknown6: f32,
    pub unknown7: f32,
    pub unknown8: f32,
    pub unknown9: f32,
    pub unknown10: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdRotationKeyFrames {
    pub version: u8,
    pub frames: Vec<CvdRotationKeyFrame>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdScaleKeyFrame {
    pub timestamp: f32,
    pub quaternion: Quaternion,
    pub scale: Vec3,
    pub unknown: [f32; 14],
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdScaleKeyFrames {
    pub version: u8,
    pub frames: Vec<CvdScaleKeyFrame>,
}

#[derive(Debug, Serialize)]
pub struct CvdModel {
    pub unknown_byte: u8,
    pub scale_factor: f32,
    pub position_keyframes: Option<CvdPositionKeyFrames>,
    pub rotation_keyframes: Option<CvdRotationKeyFrames>,
    pub scale_keyframes: Option<CvdScaleKeyFrames>,
    pub mesh: CvdMesh,
}

#[derive(Debug, Serialize)]
pub struct CvdModelNode {
    pub model: Option<CvdModel>,
    pub children: Option<Vec<CvdModelNode>>,
}

#[derive(Debug, Serialize)]
pub struct CvdFile {
    pub magic: [u8; 4],
    pub model_count: u32,
    pub models: Vec<CvdModelNode>,
}

pub fn cvd_load_from_file<P: AsRef<Path>>(
    vfs: &MiniFs,
    path: P,
) -> Result<CvdFile, Box<dyn Error>> {
    let mut reader = BufReader::new(vfs.open(&path).unwrap());
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).unwrap();

    let version = raw::CvdVersion::from_magic(magic).expect("Not a valid cvd file");
    let unknown_float = if version.has_material_extra() {
        0.5
    } else {
        0.4
    };

    let mut ani_path: PathBuf = path.as_ref().to_path_buf();
    ani_path.set_extension("ani");
    if ani_path.exists() {
        println!("Found ani file {:?} which isn't supported yet", ani_path);
    }

    let model_count = reader.read_u32::<LittleEndian>().unwrap();

    let mut models = vec![];
    for _i in 0..model_count {
        let model = cvd_load_model(&mut reader, unknown_float).unwrap();
        if let Some(model) = model {
            models.push(model);
        }
    }

    Ok(CvdFile {
        magic,
        model_count,
        models,
    })
}

pub fn cvd_load_model(
    reader: &mut dyn Read,
    unknown_float: f32,
) -> Result<Option<CvdModelNode>, Box<dyn Error>> {
    let version = raw::CvdVersion::from_legacy_float(unknown_float);
    let node = raw::read_model_node(reader, version)?;
    Ok(Some(convert_model_node(node)))
}

pub fn cvd_load_mesh(reader: &mut dyn Read, unknown_float: f32) -> Result<CvdMesh, Box<dyn Error>> {
    let version = raw::CvdVersion::from_legacy_float(unknown_float);
    let mesh = raw::read_mesh(reader, version)?;
    Ok(convert_mesh(mesh))
}

fn convert_model_node(node: raw::CvdModelNode) -> CvdModelNode {
    let model = node.model.map(convert_model);
    let children = if node.children.is_empty() {
        None
    } else {
        Some(node.children.into_iter().map(convert_model_node).collect())
    };

    CvdModelNode { model, children }
}

fn convert_model(model: raw::CvdModel) -> CvdModel {
    CvdModel {
        // The neutral parser only distinguishes "model present"/"absent" on
        // disk (any non-zero byte means "present"); `1` matches what every
        // observed file uses.
        unknown_byte: 1,
        scale_factor: model.scale_factor,
        position_keyframes: model.position_keyframes.map(convert_position_keyframes),
        rotation_keyframes: model.rotation_keyframes.map(convert_rotation_keyframes),
        scale_keyframes: model.scale_keyframes.map(convert_scale_keyframes),
        mesh: convert_mesh(model.mesh),
    }
}

fn convert_mesh(mesh: raw::CvdMesh) -> CvdMesh {
    let frames = mesh
        .frames
        .into_iter()
        .map(|frame| frame.into_iter().map(convert_vertex).collect())
        .collect();

    let materials = mesh.materials.into_iter().map(convert_material).collect();

    CvdMesh {
        frame_count: mesh.frame_count,
        vertex_count: mesh.vertex_count,
        frames,
        unknown_data: mesh.frame_extra,
        material_count: mesh.material_count,
        materials,
    }
}

fn convert_vertex(v: raw::CvdVertex) -> CvdVertex {
    let [px, py, pz] = v.position;
    let [nx, ny, nz] = v.normal;
    CvdVertex {
        position: Vec3::new(px, pz, -py),
        normal: Vec3::new(nx, ny, nz),
        tex_coord: Vec2::new(v.tex_coord.u, v.tex_coord.v),
    }
}

fn convert_material(m: raw::CvdMaterial) -> CvdMaterial {
    let triangles = if m.triangle_count > 0 {
        Some(
            m.triangles
                .into_iter()
                .map(|t| CvdTriangle { indices: t.indices })
                .collect(),
        )
    } else {
        None
    };

    CvdMaterial {
        unknown_byte: m.unknown_byte,
        color1: m.color1,
        color2: m.color2,
        color3: m.color3,
        color4: m.color4,
        texture_name: m.texture_name,
        triangle_count: m.triangle_count,
        triangles,
    }
}

fn convert_position_keyframes(kf: raw::CvdPositionKeyFrames) -> CvdPositionKeyFrames {
    let version = kf.version;
    let frames = kf
        .frames
        .into_iter()
        .map(|f| {
            let u = f.unknown;
            let mut position = match version {
                1 => Vec3::new(u[6], u[7], u[8]),
                2 => Vec3::new(u[7], u[8], u[9]),
                3 => Vec3::new(u[1], u[2], u[3]),
                _ => panic!("Unsupported position key frames version: {}", version),
            };

            std::mem::swap(&mut position.y, &mut position.z);
            position.z = -position.z;

            CvdPositionKeyFrame {
                timestamp: f.timestamp,
                position,
                unknown1: u[0],
                unknown2: u[1],
                unknown3: u[2],
                unknown4: u[3],
                unknown5: u[4],
                unknown6: u[5],
                unknown7: u[6],
                unknown8: u[7],
                unknown9: u[8],
                unknown10: u[9],
            }
        })
        .collect();

    CvdPositionKeyFrames { version, frames }
}

fn convert_rotation_keyframes(kf: raw::CvdRotationKeyFrames) -> CvdRotationKeyFrames {
    let version = kf.version;
    let frames = kf
        .frames
        .into_iter()
        .map(|f| {
            let u = f.unknown;
            let mut quaternion = match version {
                1 => Quaternion::from_axis_angle(&Vec3::new(u[6], u[7], u[8]), u[9]),
                2 | 3 => Quaternion::new(u[1], u[2], u[3], u[4]),
                _ => panic!("Unsupported position key frames version: {}", version),
            };

            std::mem::swap(&mut quaternion.y, &mut quaternion.z);
            quaternion.z = -quaternion.z;
            // CVD stores node rotations in the opposite handedness from radiance's
            // quaternion convention, so the basis-changed quaternion must be
            // inverted to rotate meshes the correct way. Without this, any node
            // whose rotation is not a 180° turn (which is inversion-invariant) is
            // mis-oriented — e.g. the Q01/Y back-door leaves end up rotated so
            // their bottom edge floats ~30 units above the threshold instead of
            // resting on it.
            quaternion.inverse();

            CvdRotationKeyFrame {
                timestamp: f.timestamp,
                quaternion,
                unknown1: u[0],
                unknown2: u[1],
                unknown3: u[2],
                unknown4: u[3],
                unknown5: u[4],
                unknown6: u[5],
                unknown7: u[6],
                unknown8: u[7],
                unknown9: u[8],
                unknown10: u[9],
            }
        })
        .collect();

    CvdRotationKeyFrames { version, frames }
}

fn convert_scale_keyframes(kf: raw::CvdScaleKeyFrames) -> CvdScaleKeyFrames {
    let version = kf.version;
    let frames = kf
        .frames
        .into_iter()
        .map(|f| {
            let unknown = f.unknown;
            let (mut quaternion, mut scale) = match version {
                1 => (
                    Quaternion::new(unknown[9], unknown[10], unknown[11], unknown[12]),
                    Vec3::new(unknown[6], unknown[7], unknown[8]),
                ),
                2 => (
                    Quaternion::new(unknown[10], unknown[11], unknown[12], unknown[13]),
                    Vec3::new(unknown[7], unknown[8], unknown[9]),
                ),
                3 => (
                    Quaternion::new(unknown[4], unknown[5], unknown[6], unknown[7]),
                    Vec3::new(unknown[1], unknown[2], unknown[3]),
                ),
                _ => panic!("Unsupported position key frames version: {}", version),
            };

            std::mem::swap(&mut quaternion.y, &mut quaternion.z);
            quaternion.z = -quaternion.z;
            std::mem::swap(&mut scale.y, &mut scale.z);
            // scale.z = -scale.z;

            CvdScaleKeyFrame {
                timestamp: f.timestamp,
                quaternion,
                scale,
                unknown,
            }
        })
        .collect();

    CvdScaleKeyFrames { version, frames }
}
