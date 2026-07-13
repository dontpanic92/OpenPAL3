//! Neutral parser/writer for PAL3's `.cvd` model format ("cvd" = "cutscene
//! vertex data"?, name unconfirmed).
//!
//! Layout (little-endian):
//!
//! ```text
//! 0x00  [u8; 4]   magic: "cvds" (has per-material extra data) or "cvdf" (no
//!                 extra data)
//! 0x04  u32       model_count
//! repeat model_count times:
//!   <model node, recursive, see `read_model_node`>
//! ```
//!
//! Every DTO in this module stores the *raw* on-disk values (no coordinate
//! system conversion, no derived fields): the CVD format encodes several
//! ambiguous/version-dependent quantities (e.g. which of ten `unknownN`
//! floats in a keyframe actually holds a position) whose *interpretation*
//! depends on the consuming engine's coordinate conventions. That
//! interpretation intentionally stays in `shared`'s adapter
//! (`openpal3::loaders::cvd_loader`) so this module can be reused
//! independent of any particular 3D engine.
use std::io::{Read, Write};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use encoding::{EncoderTrap, Encoding};
use serde::Serialize;
use thiserror::Error;

use crate::rwbs::{Matrix44f, TexCoord};
use crate::utils::to_gbk_string;

/// Fixed on-disk width (in bytes) of a [`CvdMaterial::texture_name`] field.
const TEXTURE_NAME_CAPACITY: usize = 64;

#[derive(Debug, Error)]
pub enum CvdError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("cvd magic {0:?} is not recognized (expected \"cvds\" or \"cvdf\")")]
    InvalidMagic([u8; 4]),

    #[error("unsupported {kind} keyframe version: {version}")]
    UnsupportedKeyframeVersion { kind: &'static str, version: u8 },

    #[error(
        "{field} count mismatch: header/count field says {expected}, but {actual} elements are present"
    )]
    CountMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },

    #[error(
        "cvd material texture name \"{name}\" is {actual} bytes, which exceeds the {TEXTURE_NAME_CAPACITY}-byte capacity"
    )]
    TextureNameTooLong { name: String, actual: usize },

    #[error(
        "cvd material's extra per-material data block is present ({present}), which doesn't match the file's version ({version:?}); \"cvdf\" files never carry it, \"cvds\" files always do (possibly empty)"
    )]
    MaterialExtraVersionMismatch { present: bool, version: CvdVersion },

    #[error(
        "cvd material extra data has {value_count} values but {block_count} 20-byte blocks; the two must match"
    )]
    MaterialExtraCountMismatch {
        value_count: usize,
        block_count: usize,
    },

    #[error(
        "cvd {kind} keyframes is Some(..) but has zero frames; writing count=0 with a trailing version byte would desync the reader (which returns None on count<=0 without consuming the version byte) — use None instead"
    )]
    EmptyKeyframes { kind: &'static str },
}

pub type Result<T> = std::result::Result<T, CvdError>;

/// The two known `.cvd` container versions, distinguished by magic.
///
/// The only observed behavioral difference is whether each material carries
/// an extra per-material data block (`CvdMaterial::extra`); see
/// [`CvdVersion::has_material_extra`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CvdVersion {
    /// `"cvdf"` magic. Materials never carry the extra data block.
    V1,
    /// `"cvds"` magic. Materials always carry the extra data block (which
    /// may itself be empty).
    V2,
}

impl CvdVersion {
    fn magic(self) -> [u8; 4] {
        match self {
            CvdVersion::V1 => *b"cvdf",
            CvdVersion::V2 => *b"cvds",
        }
    }

    pub fn from_magic(magic: [u8; 4]) -> Result<Self> {
        match &magic {
            b"cvdf" => Ok(CvdVersion::V1),
            b"cvds" => Ok(CvdVersion::V2),
            _ => Err(CvdError::InvalidMagic(magic)),
        }
    }

    /// Mirrors the original loader's `unknown_float >= 0.5` check, which
    /// derived the container version from an ad-hoc float (0.4 for "cvdf",
    /// 0.5 for "cvds") rather than the magic directly.
    pub fn from_legacy_float(value: f32) -> Self {
        if value >= 0.5 {
            CvdVersion::V2
        } else {
            CvdVersion::V1
        }
    }

    pub fn has_material_extra(self) -> bool {
        matches!(self, CvdVersion::V2)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdVertex {
    /// Texture `u`/`v`.
    pub tex_coord: TexCoord,
    /// Raw normal (`nx`, `ny`, `nz`) exactly as stored on disk.
    pub normal: [f32; 3],
    /// Raw position (`px`, `py`, `pz`) exactly as stored on disk.
    pub position: [f32; 3],
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdTriangle {
    pub indices: [u16; 3],
}

/// The extra per-material data block only present in `CvdVersion::V2`
/// ("cvds") files. `values.len()` always equals `blocks.len()`; the
/// original loader read (and discarded) this data without documenting its
/// purpose.
#[derive(Debug, Clone, Serialize)]
pub struct CvdMaterialExtra {
    pub values: Vec<u32>,
    pub blocks: Vec<[u8; 20]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdMaterial {
    pub unknown_byte: u8,
    pub color1: u32,
    pub color2: u32,
    pub color3: u32,
    pub color4: u32,
    /// Read directly after `color4`; the original loader decoded and then
    /// discarded this value (`_unknown_float2`). Kept here for a lossless
    /// round trip.
    pub unknown_float2: f32,
    pub texture_name: String,
    pub triangle_count: u32,
    pub triangles: Vec<CvdTriangle>,
    /// `Some` (possibly with empty `values`/`blocks`) iff the containing
    /// file is [`CvdVersion::V2`].
    pub extra: Option<CvdMaterialExtra>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdMesh {
    pub frame_count: u32,
    pub vertex_count: u32,
    pub frames: Vec<Vec<CvdVertex>>,
    /// One `f32` per frame (originally read into `unknown_data`); purpose
    /// unconfirmed.
    pub frame_extra: Vec<f32>,
    pub material_count: u32,
    pub materials: Vec<CvdMaterial>,
}

/// A position keyframe. `unknown` holds the raw `unknown1..unknown10`
/// floats read from disk; which of them represent the actual position (and
/// what axis swap/negation to apply) depends on `version` and is decided by
/// the consumer (see `CvdPositionKeyFrame::position` in
/// `shared::openpal3::loaders::cvd_loader` for the existing interpretation).
#[derive(Debug, Clone, Serialize)]
pub struct CvdPositionKeyFrame {
    pub timestamp: f32,
    pub unknown: [f32; 10],
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdPositionKeyFrames {
    pub version: u8,
    pub frames: Vec<CvdPositionKeyFrame>,
}

/// A rotation keyframe; see [`CvdPositionKeyFrame`] for why `unknown` is
/// left uninterpreted here.
#[derive(Debug, Clone, Serialize)]
pub struct CvdRotationKeyFrame {
    pub timestamp: f32,
    pub unknown: [f32; 10],
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdRotationKeyFrames {
    pub version: u8,
    pub frames: Vec<CvdRotationKeyFrame>,
}

/// A scale keyframe; see [`CvdPositionKeyFrame`] for why `unknown` is left
/// uninterpreted here.
#[derive(Debug, Clone, Serialize)]
pub struct CvdScaleKeyFrame {
    pub timestamp: f32,
    pub unknown: [f32; 14],
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdScaleKeyFrames {
    pub version: u8,
    pub frames: Vec<CvdScaleKeyFrame>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdModel {
    pub position_keyframes: Option<CvdPositionKeyFrames>,
    pub rotation_keyframes: Option<CvdRotationKeyFrames>,
    pub scale_keyframes: Option<CvdScaleKeyFrames>,
    pub scale_factor: f32,
    pub mesh: CvdMesh,
    /// Row-major 4x4 transform trailing the mesh data.
    pub matrix: Matrix44f,
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdModelNode {
    pub model: Option<CvdModel>,
    pub children: Vec<CvdModelNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CvdFile {
    pub version: CvdVersion,
    pub models: Vec<CvdModelNode>,
}

fn check_count(field: &'static str, expected: u32, actual: usize) -> Result<()> {
    if expected as usize != actual {
        return Err(CvdError::CountMismatch {
            field,
            expected: expected as usize,
            actual,
        });
    }
    Ok(())
}

fn decode_texture_name(bytes: &[u8; TEXTURE_NAME_CAPACITY]) -> String {
    let end = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
    to_gbk_string(&bytes[..end]).unwrap_or_else(|_| {
        log::error!("Failed to decode cvd texture name: {:?}", &bytes[..end]);
        String::new()
    })
}

/// Encodes `name` with GBK and pads/validates it against
/// [`TEXTURE_NAME_CAPACITY`]. Returns an error instead of silently
/// truncating if it doesn't fit.
fn encode_texture_name(name: &str) -> Result<[u8; TEXTURE_NAME_CAPACITY]> {
    let encoded = encoding::all::GBK
        .encode(name, EncoderTrap::Strict)
        .map_err(|_| CvdError::TextureNameTooLong {
            name: name.to_string(),
            actual: name.len(),
        })?;

    if encoded.len() > TEXTURE_NAME_CAPACITY {
        return Err(CvdError::TextureNameTooLong {
            name: name.to_string(),
            actual: encoded.len(),
        });
    }

    let mut bytes = [0u8; TEXTURE_NAME_CAPACITY];
    bytes[..encoded.len()].copy_from_slice(&encoded);
    Ok(bytes)
}

fn read_vertex(reader: &mut (impl Read + ?Sized)) -> Result<CvdVertex> {
    let tx = reader.read_f32::<LittleEndian>()?;
    let ty = reader.read_f32::<LittleEndian>()?;
    let nx = reader.read_f32::<LittleEndian>()?;
    let ny = reader.read_f32::<LittleEndian>()?;
    let nz = reader.read_f32::<LittleEndian>()?;
    let px = reader.read_f32::<LittleEndian>()?;
    let py = reader.read_f32::<LittleEndian>()?;
    let pz = reader.read_f32::<LittleEndian>()?;
    Ok(CvdVertex {
        tex_coord: TexCoord { u: tx, v: ty },
        normal: [nx, ny, nz],
        position: [px, py, pz],
    })
}

fn write_vertex(writer: &mut impl Write, vertex: &CvdVertex) -> Result<()> {
    writer.write_f32::<LittleEndian>(vertex.tex_coord.u)?;
    writer.write_f32::<LittleEndian>(vertex.tex_coord.v)?;
    for v in vertex.normal {
        writer.write_f32::<LittleEndian>(v)?;
    }
    for v in vertex.position {
        writer.write_f32::<LittleEndian>(v)?;
    }
    Ok(())
}

fn read_position_keyframes(
    reader: &mut (impl Read + ?Sized),
) -> Result<Option<CvdPositionKeyFrames>> {
    let count = reader.read_i32::<LittleEndian>()?;
    if count <= 0 {
        return Ok(None);
    }

    let version = reader.read_u8()?;
    if !(1..=3).contains(&version) {
        return Err(CvdError::UnsupportedKeyframeVersion {
            kind: "position",
            version,
        });
    }

    let mut frames = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let timestamp = reader.read_f32::<LittleEndian>()?;
        let mut unknown = [0f32; 10];
        reader.read_f32_into::<LittleEndian>(&mut unknown)?;
        frames.push(CvdPositionKeyFrame { timestamp, unknown });
    }

    Ok(Some(CvdPositionKeyFrames { version, frames }))
}

fn write_position_keyframes(
    writer: &mut impl Write,
    keyframes: &Option<CvdPositionKeyFrames>,
) -> Result<()> {
    match keyframes {
        None => {
            writer.write_i32::<LittleEndian>(0)?;
        }
        Some(kf) => {
            if kf.frames.is_empty() {
                return Err(CvdError::EmptyKeyframes { kind: "position" });
            }
            if !(1..=3).contains(&kf.version) {
                return Err(CvdError::UnsupportedKeyframeVersion {
                    kind: "position",
                    version: kf.version,
                });
            }

            writer.write_i32::<LittleEndian>(kf.frames.len() as i32)?;
            writer.write_u8(kf.version)?;
            for frame in &kf.frames {
                writer.write_f32::<LittleEndian>(frame.timestamp)?;
                for v in frame.unknown {
                    writer.write_f32::<LittleEndian>(v)?;
                }
            }
        }
    }
    Ok(())
}

fn read_rotation_keyframes(
    reader: &mut (impl Read + ?Sized),
) -> Result<Option<CvdRotationKeyFrames>> {
    let count = reader.read_i32::<LittleEndian>()?;
    if count <= 0 {
        return Ok(None);
    }

    let version = reader.read_u8()?;
    if !(1..=3).contains(&version) {
        return Err(CvdError::UnsupportedKeyframeVersion {
            kind: "rotation",
            version,
        });
    }

    let mut frames = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let timestamp = reader.read_f32::<LittleEndian>()?;
        let mut unknown = [0f32; 10];
        reader.read_f32_into::<LittleEndian>(&mut unknown)?;
        frames.push(CvdRotationKeyFrame { timestamp, unknown });
    }

    Ok(Some(CvdRotationKeyFrames { version, frames }))
}

fn write_rotation_keyframes(
    writer: &mut impl Write,
    keyframes: &Option<CvdRotationKeyFrames>,
) -> Result<()> {
    match keyframes {
        None => {
            writer.write_i32::<LittleEndian>(0)?;
        }
        Some(kf) => {
            if kf.frames.is_empty() {
                return Err(CvdError::EmptyKeyframes { kind: "rotation" });
            }
            if !(1..=3).contains(&kf.version) {
                return Err(CvdError::UnsupportedKeyframeVersion {
                    kind: "rotation",
                    version: kf.version,
                });
            }

            writer.write_i32::<LittleEndian>(kf.frames.len() as i32)?;
            writer.write_u8(kf.version)?;
            for frame in &kf.frames {
                writer.write_f32::<LittleEndian>(frame.timestamp)?;
                for v in frame.unknown {
                    writer.write_f32::<LittleEndian>(v)?;
                }
            }
        }
    }
    Ok(())
}

fn read_scale_keyframes(reader: &mut (impl Read + ?Sized)) -> Result<Option<CvdScaleKeyFrames>> {
    let count = reader.read_i32::<LittleEndian>()?;
    if count <= 0 {
        return Ok(None);
    }

    let version = reader.read_u8()?;
    if !(1..=3).contains(&version) {
        return Err(CvdError::UnsupportedKeyframeVersion {
            kind: "scale",
            version,
        });
    }

    let mut frames = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let timestamp = reader.read_f32::<LittleEndian>()?;
        let mut unknown = [0f32; 14];
        reader.read_f32_into::<LittleEndian>(&mut unknown)?;
        frames.push(CvdScaleKeyFrame { timestamp, unknown });
    }

    Ok(Some(CvdScaleKeyFrames { version, frames }))
}

fn write_scale_keyframes(
    writer: &mut impl Write,
    keyframes: &Option<CvdScaleKeyFrames>,
) -> Result<()> {
    match keyframes {
        None => {
            writer.write_i32::<LittleEndian>(0)?;
        }
        Some(kf) => {
            if kf.frames.is_empty() {
                return Err(CvdError::EmptyKeyframes { kind: "scale" });
            }
            if !(1..=3).contains(&kf.version) {
                return Err(CvdError::UnsupportedKeyframeVersion {
                    kind: "scale",
                    version: kf.version,
                });
            }

            writer.write_i32::<LittleEndian>(kf.frames.len() as i32)?;
            writer.write_u8(kf.version)?;
            for frame in &kf.frames {
                writer.write_f32::<LittleEndian>(frame.timestamp)?;
                for v in frame.unknown {
                    writer.write_f32::<LittleEndian>(v)?;
                }
            }
        }
    }
    Ok(())
}

/// Read a single material, matching `cvd_load_mesh`'s per-material loop in
/// the original loader.
fn read_material(reader: &mut (impl Read + ?Sized), version: CvdVersion) -> Result<CvdMaterial> {
    let unknown_byte = reader.read_u8()?;
    let color1 = reader.read_u32::<LittleEndian>()?;
    let color2 = reader.read_u32::<LittleEndian>()?;
    let color3 = reader.read_u32::<LittleEndian>()?;
    let color4 = reader.read_u32::<LittleEndian>()?;
    let unknown_float2 = reader.read_f32::<LittleEndian>()?;

    let mut name_bytes = [0u8; TEXTURE_NAME_CAPACITY];
    reader.read_exact(&mut name_bytes)?;
    let texture_name = decode_texture_name(&name_bytes);

    let triangle_count = reader.read_u32::<LittleEndian>()?;
    let mut triangles = Vec::with_capacity(triangle_count as usize);
    for _ in 0..triangle_count {
        let i0 = reader.read_u16::<LittleEndian>()?;
        let i1 = reader.read_u16::<LittleEndian>()?;
        let i2 = reader.read_u16::<LittleEndian>()?;
        triangles.push(CvdTriangle {
            indices: [i0, i1, i2],
        });
    }

    let extra = if version.has_material_extra() {
        let count = reader.read_u32::<LittleEndian>()?;
        let mut values = Vec::with_capacity(count as usize);
        for _ in 0..count {
            values.push(reader.read_u32::<LittleEndian>()?);
        }

        let mut blocks = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let mut block = [0u8; 20];
            reader.read_exact(&mut block)?;
            blocks.push(block);
        }

        Some(CvdMaterialExtra { values, blocks })
    } else {
        None
    };

    Ok(CvdMaterial {
        unknown_byte,
        color1,
        color2,
        color3,
        color4,
        unknown_float2,
        texture_name,
        triangle_count,
        triangles,
        extra,
    })
}

fn write_material(
    writer: &mut impl Write,
    material: &CvdMaterial,
    version: CvdVersion,
) -> Result<()> {
    check_count(
        "material.triangle_count",
        material.triangle_count,
        material.triangles.len(),
    )?;

    if material.extra.is_some() != version.has_material_extra() {
        return Err(CvdError::MaterialExtraVersionMismatch {
            present: material.extra.is_some(),
            version,
        });
    }

    writer.write_u8(material.unknown_byte)?;
    writer.write_u32::<LittleEndian>(material.color1)?;
    writer.write_u32::<LittleEndian>(material.color2)?;
    writer.write_u32::<LittleEndian>(material.color3)?;
    writer.write_u32::<LittleEndian>(material.color4)?;
    writer.write_f32::<LittleEndian>(material.unknown_float2)?;
    writer.write_all(&encode_texture_name(&material.texture_name)?)?;

    writer.write_u32::<LittleEndian>(material.triangle_count)?;
    for triangle in &material.triangles {
        for i in triangle.indices {
            writer.write_u16::<LittleEndian>(i)?;
        }
    }

    if let Some(extra) = &material.extra {
        if extra.values.len() != extra.blocks.len() {
            return Err(CvdError::MaterialExtraCountMismatch {
                value_count: extra.values.len(),
                block_count: extra.blocks.len(),
            });
        }

        writer.write_u32::<LittleEndian>(extra.values.len() as u32)?;
        for v in &extra.values {
            writer.write_u32::<LittleEndian>(*v)?;
        }
        for block in &extra.blocks {
            writer.write_all(block)?;
        }
    }

    Ok(())
}

/// Read a mesh, matching `cvd_load_mesh` in the original loader.
pub fn read_mesh(reader: &mut (impl Read + ?Sized), version: CvdVersion) -> Result<CvdMesh> {
    let frame_count = reader.read_u32::<LittleEndian>()?;
    let vertex_count = reader.read_u32::<LittleEndian>()?;

    let mut frames = Vec::with_capacity(frame_count as usize);
    for _ in 0..frame_count {
        let mut vertices = Vec::with_capacity(vertex_count as usize);
        for _ in 0..vertex_count {
            vertices.push(read_vertex(reader)?);
        }
        frames.push(vertices);
    }

    let mut frame_extra = vec![0f32; frame_count as usize];
    reader.read_f32_into::<LittleEndian>(&mut frame_extra)?;

    let material_count = reader.read_u32::<LittleEndian>()?;
    let mut materials = Vec::with_capacity(material_count as usize);
    for _ in 0..material_count {
        materials.push(read_material(reader, version)?);
    }

    Ok(CvdMesh {
        frame_count,
        vertex_count,
        frames,
        frame_extra,
        material_count,
        materials,
    })
}

/// Validate a [`CvdMesh`]'s internal counts without writing it.
pub fn validate_mesh(mesh: &CvdMesh, version: CvdVersion) -> Result<()> {
    check_count("mesh.frame_count", mesh.frame_count, mesh.frames.len())?;
    for frame in &mesh.frames {
        check_count("mesh.vertex_count", mesh.vertex_count, frame.len())?;
    }
    check_count("mesh.frame_extra", mesh.frame_count, mesh.frame_extra.len())?;
    check_count(
        "mesh.material_count",
        mesh.material_count,
        mesh.materials.len(),
    )?;

    for material in &mesh.materials {
        check_count(
            "material.triangle_count",
            material.triangle_count,
            material.triangles.len(),
        )?;

        if material.extra.is_some() != version.has_material_extra() {
            return Err(CvdError::MaterialExtraVersionMismatch {
                present: material.extra.is_some(),
                version,
            });
        }

        if let Some(extra) = &material.extra {
            if extra.values.len() != extra.blocks.len() {
                return Err(CvdError::MaterialExtraCountMismatch {
                    value_count: extra.values.len(),
                    block_count: extra.blocks.len(),
                });
            }
        }
    }

    Ok(())
}

/// Write a mesh, matching the exact binary layout read by [`read_mesh`].
pub fn write_mesh(writer: &mut impl Write, mesh: &CvdMesh, version: CvdVersion) -> Result<()> {
    validate_mesh(mesh, version)?;

    writer.write_u32::<LittleEndian>(mesh.frame_count)?;
    writer.write_u32::<LittleEndian>(mesh.vertex_count)?;
    for frame in &mesh.frames {
        for vertex in frame {
            write_vertex(writer, vertex)?;
        }
    }

    for v in &mesh.frame_extra {
        writer.write_f32::<LittleEndian>(*v)?;
    }

    writer.write_u32::<LittleEndian>(mesh.material_count)?;
    for material in &mesh.materials {
        write_material(writer, material, version)?;
    }

    Ok(())
}

/// Read a model node (a model plus its children), matching `cvd_load_model`
/// in the original loader.
pub fn read_model_node(
    reader: &mut (impl Read + ?Sized),
    version: CvdVersion,
) -> Result<CvdModelNode> {
    let unknown_byte = reader.read_u8()?;

    let model = if unknown_byte > 0 {
        let position_keyframes = read_position_keyframes(reader)?;
        let rotation_keyframes = read_rotation_keyframes(reader)?;
        let scale_keyframes = read_scale_keyframes(reader)?;
        let scale_factor = reader.read_f32::<LittleEndian>()?;
        let mesh = read_mesh(reader, version)?;

        let mut floats = [0f32; 16];
        reader.read_f32_into::<LittleEndian>(&mut floats)?;

        Some(CvdModel {
            position_keyframes,
            rotation_keyframes,
            scale_keyframes,
            scale_factor,
            mesh,
            matrix: Matrix44f(floats),
        })
    } else {
        None
    };

    let children_count = reader.read_u32::<LittleEndian>()?;
    let mut children = Vec::with_capacity(children_count as usize);
    for _ in 0..children_count {
        children.push(read_model_node(reader, version)?);
    }

    Ok(CvdModelNode { model, children })
}

/// Write a model node, matching the exact binary layout read by
/// [`read_model_node`].
pub fn write_model_node(
    writer: &mut impl Write,
    node: &CvdModelNode,
    version: CvdVersion,
) -> Result<()> {
    match &node.model {
        None => {
            writer.write_u8(0)?;
        }
        Some(model) => {
            // The original format doesn't store `unknown_byte` verbatim; it
            // only distinguishes "has a model" (any value > 0) from "no
            // model" (0). We always emit `1` for "has a model", matching
            // the vast majority of real files.
            writer.write_u8(1)?;
            write_position_keyframes(writer, &model.position_keyframes)?;
            write_rotation_keyframes(writer, &model.rotation_keyframes)?;
            write_scale_keyframes(writer, &model.scale_keyframes)?;
            writer.write_f32::<LittleEndian>(model.scale_factor)?;
            write_mesh(writer, &model.mesh, version)?;
            for v in model.matrix.0 {
                writer.write_f32::<LittleEndian>(v)?;
            }
        }
    }

    writer.write_u32::<LittleEndian>(node.children.len() as u32)?;
    for child in &node.children {
        write_model_node(writer, child, version)?;
    }

    Ok(())
}

/// Parse a complete `.cvd` file from `reader`.
pub fn read_cvd(reader: &mut (impl Read + ?Sized)) -> Result<CvdFile> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    let version = CvdVersion::from_magic(magic)?;

    let model_count = reader.read_u32::<LittleEndian>()?;
    let mut models = Vec::with_capacity(model_count as usize);
    for _ in 0..model_count {
        models.push(read_model_node(reader, version)?);
    }

    Ok(CvdFile { version, models })
}

/// Validate a [`CvdFile`]'s internal counts without writing it.
pub fn validate_cvd(file: &CvdFile) -> Result<()> {
    fn validate_node(node: &CvdModelNode, version: CvdVersion) -> Result<()> {
        if let Some(model) = &node.model {
            validate_mesh(&model.mesh, version)?;
        }
        for child in &node.children {
            validate_node(child, version)?;
        }
        Ok(())
    }

    for model in &file.models {
        validate_node(model, file.version)?;
    }

    Ok(())
}

/// Serialize a [`CvdFile`] to `writer`, matching the exact binary layout
/// produced by [`read_cvd`].
///
/// Every count embedded in the format (frame/vertex/material/triangle
/// counts, the per-material extra-data block's presence, ...) is validated
/// against the actual data first; see [`validate_cvd`] and [`CvdError`] for
/// the specific checks.
pub fn write_cvd(writer: &mut impl Write, file: &CvdFile) -> Result<()> {
    validate_cvd(file)?;

    writer.write_all(&file.version.magic())?;
    writer.write_u32::<LittleEndian>(file.models.len() as u32)?;
    for model in &file.models {
        write_model_node(writer, model, file.version)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn sample_vertex(seed: f32) -> CvdVertex {
        CvdVertex {
            tex_coord: TexCoord {
                u: seed,
                v: seed + 0.1,
            },
            normal: [seed + 0.2, seed + 0.3, seed + 0.4],
            position: [seed + 0.5, seed + 0.6, seed + 0.7],
        }
    }

    fn sample_material(with_extra: bool) -> CvdMaterial {
        CvdMaterial {
            unknown_byte: 1,
            color1: 0xff0000ff,
            color2: 0x00ff00ff,
            color3: 0x0000ffff,
            color4: 0xffffffff,
            unknown_float2: 1.5,
            texture_name: "床边.bmp".to_string(),
            triangle_count: 1,
            triangles: vec![CvdTriangle { indices: [0, 1, 2] }],
            extra: if with_extra {
                Some(CvdMaterialExtra {
                    values: vec![1, 2],
                    blocks: vec![[7u8; 20], [8u8; 20]],
                })
            } else {
                None
            },
        }
    }

    fn sample_mesh(version: CvdVersion) -> CvdMesh {
        CvdMesh {
            frame_count: 2,
            vertex_count: 3,
            frames: vec![
                vec![sample_vertex(0.0), sample_vertex(1.0), sample_vertex(2.0)],
                vec![sample_vertex(3.0), sample_vertex(4.0), sample_vertex(5.0)],
            ],
            frame_extra: vec![0.0, 1.0],
            material_count: 1,
            materials: vec![sample_material(version.has_material_extra())],
        }
    }

    fn sample_file(version: CvdVersion) -> CvdFile {
        let leaf = CvdModelNode {
            model: Some(CvdModel {
                position_keyframes: Some(CvdPositionKeyFrames {
                    version: 3,
                    frames: vec![CvdPositionKeyFrame {
                        timestamp: 0.0,
                        unknown: [1.0; 10],
                    }],
                }),
                rotation_keyframes: Some(CvdRotationKeyFrames {
                    version: 2,
                    frames: vec![CvdRotationKeyFrame {
                        timestamp: 0.0,
                        unknown: [2.0; 10],
                    }],
                }),
                scale_keyframes: None,
                scale_factor: 1.0,
                mesh: sample_mesh(version),
                matrix: Matrix44f([0.0; 16]),
            }),
            children: vec![],
        };

        let root = CvdModelNode {
            model: None,
            children: vec![leaf],
        };

        CvdFile {
            version,
            models: vec![root],
        }
    }

    #[test]
    fn parse_write_parse_round_trip_v1() {
        let original = sample_file(CvdVersion::V1);

        let mut buf = Cursor::new(Vec::new());
        write_cvd(&mut buf, &original).expect("write_cvd should succeed for a valid v1 file");

        buf.set_position(0);
        let roundtripped = read_cvd(&mut buf).expect("re-parsing the written buffer should work");

        assert_eq!(format!("{:?}", original), format!("{:?}", roundtripped));
    }

    #[test]
    fn parse_write_parse_round_trip_v2_with_extra_material_data() {
        let original = sample_file(CvdVersion::V2);

        let mut buf = Cursor::new(Vec::new());
        write_cvd(&mut buf, &original).expect("write_cvd should succeed for a valid v2 file");

        buf.set_position(0);
        let roundtripped = read_cvd(&mut buf).expect("re-parsing the written buffer should work");

        assert_eq!(format!("{:?}", original), format!("{:?}", roundtripped));
    }

    #[test]
    fn write_rejects_frame_vertex_count_mismatch() {
        let mut file = sample_file(CvdVersion::V1);
        file.models[0].children[0]
            .model
            .as_mut()
            .unwrap()
            .mesh
            .frames[0]
            .pop();

        let mut buf = Cursor::new(Vec::new());
        let err = write_cvd(&mut buf, &file).unwrap_err();
        assert!(matches!(
            err,
            CvdError::CountMismatch {
                field: "mesh.vertex_count",
                expected: 3,
                actual: 2,
            }
        ));
    }

    #[test]
    fn write_rejects_material_extra_version_mismatch() {
        // V1 files must never carry the extra per-material data block.
        let mut file = sample_file(CvdVersion::V1);
        file.models[0].children[0]
            .model
            .as_mut()
            .unwrap()
            .mesh
            .materials[0]
            .extra = Some(CvdMaterialExtra {
            values: vec![1],
            blocks: vec![[0u8; 20]],
        });

        let mut buf = Cursor::new(Vec::new());
        let err = write_cvd(&mut buf, &file).unwrap_err();
        assert!(matches!(
            err,
            CvdError::MaterialExtraVersionMismatch {
                present: true,
                version: CvdVersion::V1,
            }
        ));
    }

    #[test]
    fn write_rejects_unsupported_keyframe_version() {
        let mut file = sample_file(CvdVersion::V1);
        file.models[0].children[0]
            .model
            .as_mut()
            .unwrap()
            .position_keyframes
            .as_mut()
            .unwrap()
            .version = 9;

        let mut buf = Cursor::new(Vec::new());
        let err = write_cvd(&mut buf, &file).unwrap_err();
        assert!(matches!(
            err,
            CvdError::UnsupportedKeyframeVersion {
                kind: "position",
                version: 9,
            }
        ));
    }

    #[test]
    fn write_rejects_texture_name_too_long() {
        let mut file = sample_file(CvdVersion::V1);
        file.models[0].children[0]
            .model
            .as_mut()
            .unwrap()
            .mesh
            .materials[0]
            .texture_name = "a".repeat(TEXTURE_NAME_CAPACITY + 1);

        let mut buf = Cursor::new(Vec::new());
        let err = write_cvd(&mut buf, &file).unwrap_err();
        assert!(matches!(err, CvdError::TextureNameTooLong { .. }));
    }

    #[test]
    fn write_rejects_empty_position_keyframes() {
        let mut file = sample_file(CvdVersion::V1);
        file.models[0].children[0]
            .model
            .as_mut()
            .unwrap()
            .position_keyframes = Some(CvdPositionKeyFrames {
            version: 3,
            frames: vec![],
        });

        let mut buf = Cursor::new(Vec::new());
        let err = write_cvd(&mut buf, &file).unwrap_err();
        assert!(matches!(err, CvdError::EmptyKeyframes { kind: "position" }));
    }

    #[test]
    fn write_rejects_empty_rotation_keyframes() {
        let mut file = sample_file(CvdVersion::V1);
        file.models[0].children[0]
            .model
            .as_mut()
            .unwrap()
            .rotation_keyframes = Some(CvdRotationKeyFrames {
            version: 2,
            frames: vec![],
        });

        let mut buf = Cursor::new(Vec::new());
        let err = write_cvd(&mut buf, &file).unwrap_err();
        assert!(matches!(err, CvdError::EmptyKeyframes { kind: "rotation" }));
    }

    #[test]
    fn write_rejects_empty_scale_keyframes() {
        let mut file = sample_file(CvdVersion::V1);
        file.models[0].children[0]
            .model
            .as_mut()
            .unwrap()
            .scale_keyframes = Some(CvdScaleKeyFrames {
            version: 1,
            frames: vec![],
        });

        let mut buf = Cursor::new(Vec::new());
        let err = write_cvd(&mut buf, &file).unwrap_err();
        assert!(matches!(err, CvdError::EmptyKeyframes { kind: "scale" }));
    }
}
