use binrw::{BinRead, BinWrite};
use serde::Serialize;
use std::io::{Read, Seek, Write};
use thiserror::Error;

use crate::{
    rwbs::TexCoord,
    utils::{SizedString, StringWithCapacity},
};

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3Texture {
    #[br(count = 17)]
    pub unknown: Vec<f32>,
    #[br(count = 4)]
    pub names: Vec<SizedString>,
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3Vertex {
    #[br(map = |v: i16| -v)]
    #[bw(map = |v: &i16| -*v)]
    pub x: i16,
    pub y: i16,
    #[br(map = |v: i16| -v)]
    #[bw(map = |v: &i16| -*v)]
    pub z: i16,
    pub normal_phi: i8,
    pub normal_theta: u8,
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little, import(count: u32))]
pub struct Mv3Frame {
    pub timestamp: u32,
    #[br(count = count)]
    pub vertices: Vec<Mv3Vertex>,
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3Triangle {
    pub indices: [u16; 3],
    pub texcoord_indices: [u16; 3],
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3UnknownDataInMesh {
    pub u: u16,
    pub v: u16,
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3Mesh {
    pub unknown: u32,
    pub triangle_count: u32,
    #[br(count = triangle_count)]
    pub triangles: Vec<Mv3Triangle>,
    pub unknown_data_count: u32,
    #[br(count = unknown_data_count)]
    pub unknown_data: Vec<Mv3UnknownDataInMesh>,
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3Model {
    #[br(count = 64)]
    pub unknown: Vec<u8>,
    pub vertex_per_frame: u32,
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],
    pub frame_count: u32,
    #[br(count = frame_count, args { inner: (vertex_per_frame,) })]
    #[bw(args(*vertex_per_frame))]
    pub frames: Vec<Mv3Frame>,
    pub texcoord_count: u32,
    #[br(count = texcoord_count)]
    pub texcoords: Vec<TexCoord>,
    pub mesh_count: u32,
    #[br(count = mesh_count)]
    pub meshes: Vec<Mv3Mesh>,
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3ActionDesc {
    pub tick: u32,
    #[brw(args(16))]
    pub name: StringWithCapacity,
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little)]
pub struct Mv3UnknownDataInFile {
    #[br(count = 64)]
    pub unknown0: Vec<u8>,
    pub unknown1: u32,
    pub unknown2_count: u32,
    #[br(count = unknown2_count)]
    pub unknown2: Vec<[f32; 17]>,
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little, magic = b"MV3\0")]
pub struct Mv3File {
    pub version: u32,
    pub duration: u32,

    pub texture_count: u32,
    pub unknown_data_count: u32,
    pub model_count: u32,
    pub action_count: u32,

    #[br(count = action_count)]
    pub action_desc: Vec<Mv3ActionDesc>,
    #[br(count = unknown_data_count)]
    pub unknown_data: Vec<Mv3UnknownDataInFile>,
    #[br(count = texture_count)]
    pub textures: Vec<Mv3Texture>,
    #[br(count = model_count)]
    pub models: Vec<Mv3Model>,
}

pub fn read_mv3(reader: &mut (impl Read + Seek)) -> anyhow::Result<Mv3File> {
    Ok(Mv3File::read(reader)?)
}

/// Errors produced while validating an [`Mv3File`] before it is serialized.
///
/// The MV3 format stores explicit element counts (`texture_count`,
/// `model_count`, ...) alongside the vectors they describe, and several
/// nested structures share a count across siblings (e.g. every
/// [`Mv3Frame`] in a model must carry exactly `vertex_per_frame` vertices).
/// [`write_mv3`] checks all of these invariants up front so a malformed
/// in-memory tree is rejected with a precise error instead of silently
/// producing a corrupt file.
#[derive(Debug, Error)]
pub enum Mv3WriteError {
    #[error(
        "MV3 {field} count mismatch: header/count field says {expected}, but {actual} elements are present"
    )]
    CountMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },

    #[error("MV3 texture #{index} must have exactly 17 unknown floats, got {actual}")]
    InvalidTextureUnknownLen { index: usize, actual: usize },

    #[error("MV3 texture #{index} must have exactly 4 names, got {actual}")]
    InvalidTextureNameCount { index: usize, actual: usize },

    #[error("MV3 model #{index} unknown header must be exactly 64 bytes, got {actual}")]
    InvalidModelUnknownLen { index: usize, actual: usize },

    #[error(
        "MV3 model #{model_index} frame #{frame_index} has {actual} vertices, expected vertex_per_frame = {expected}"
    )]
    FrameVertexCountMismatch {
        model_index: usize,
        frame_index: usize,
        expected: usize,
        actual: usize,
    },

    #[error(
        "MV3 model #{index} has an invalid AABB: min ({min:?}) is greater than max ({max:?}) on axis {axis}"
    )]
    InvalidAabb {
        index: usize,
        axis: usize,
        min: f32,
        max: f32,
    },

    #[error("MV3 unknown_data #{index} unknown0 must be exactly 64 bytes, got {actual}")]
    InvalidUnknownDataLen { index: usize, actual: usize },

    #[error(
        "MV3 action #{index} name \"{name}\" is {actual} bytes, which exceeds the 16 byte capacity"
    )]
    ActionNameTooLong {
        index: usize,
        name: String,
        actual: usize,
    },

    #[error("failed to serialize MV3 file: {0}")]
    Binrw(#[from] binrw::Error),
}

fn check_count(field: &'static str, expected: u32, actual: usize) -> Result<(), Mv3WriteError> {
    if expected as usize != actual {
        return Err(Mv3WriteError::CountMismatch {
            field,
            expected: expected as usize,
            actual,
        });
    }
    Ok(())
}

/// Validate an [`Mv3File`]'s internal counts and bounds without writing it.
///
/// Useful to check a tree built in memory (e.g. by an editor or converter)
/// before attempting to serialize it.
pub fn validate_mv3(file: &Mv3File) -> Result<(), Mv3WriteError> {
    check_count("texture_count", file.texture_count, file.textures.len())?;
    check_count(
        "unknown_data_count",
        file.unknown_data_count,
        file.unknown_data.len(),
    )?;
    check_count("model_count", file.model_count, file.models.len())?;
    check_count("action_count", file.action_count, file.action_desc.len())?;

    for (index, texture) in file.textures.iter().enumerate() {
        if texture.unknown.len() != 17 {
            return Err(Mv3WriteError::InvalidTextureUnknownLen {
                index,
                actual: texture.unknown.len(),
            });
        }

        if texture.names.len() != 4 {
            return Err(Mv3WriteError::InvalidTextureNameCount {
                index,
                actual: texture.names.len(),
            });
        }
    }

    for (index, unknown_data) in file.unknown_data.iter().enumerate() {
        if unknown_data.unknown0.len() != 64 {
            return Err(Mv3WriteError::InvalidUnknownDataLen {
                index,
                actual: unknown_data.unknown0.len(),
            });
        }

        check_count(
            "unknown_data.unknown2_count",
            unknown_data.unknown2_count,
            unknown_data.unknown2.len(),
        )?;
    }

    for (index, action) in file.action_desc.iter().enumerate() {
        let name_len = action.name.data().len();
        if name_len > 16 {
            return Err(Mv3WriteError::ActionNameTooLong {
                index,
                name: action.name.as_str().unwrap_or_default(),
                actual: name_len,
            });
        }
    }

    for (model_index, model) in file.models.iter().enumerate() {
        if model.unknown.len() != 64 {
            return Err(Mv3WriteError::InvalidModelUnknownLen {
                index: model_index,
                actual: model.unknown.len(),
            });
        }

        for axis in 0..3 {
            if model.aabb_min[axis] > model.aabb_max[axis] {
                return Err(Mv3WriteError::InvalidAabb {
                    index: model_index,
                    axis,
                    min: model.aabb_min[axis],
                    max: model.aabb_max[axis],
                });
            }
        }

        check_count("model.frame_count", model.frame_count, model.frames.len())?;
        for (frame_index, frame) in model.frames.iter().enumerate() {
            if frame.vertices.len() != model.vertex_per_frame as usize {
                return Err(Mv3WriteError::FrameVertexCountMismatch {
                    model_index,
                    frame_index,
                    expected: model.vertex_per_frame as usize,
                    actual: frame.vertices.len(),
                });
            }
        }

        check_count(
            "model.texcoord_count",
            model.texcoord_count,
            model.texcoords.len(),
        )?;
        check_count("model.mesh_count", model.mesh_count, model.meshes.len())?;

        for mesh in model.meshes.iter() {
            check_count(
                "mesh.triangle_count",
                mesh.triangle_count,
                mesh.triangles.len(),
            )?;
            check_count(
                "mesh.unknown_data_count",
                mesh.unknown_data_count,
                mesh.unknown_data.len(),
            )?;
        }
    }

    Ok(())
}

/// Serialize an [`Mv3File`] to `writer`, matching the exact binary layout
/// produced by [`read_mv3`].
///
/// All the count fields embedded in the format (texture/model/action counts,
/// per-model frame/texcoord/mesh counts, per-mesh triangle counts, ...) and
/// the AABB bounds are validated against the actual vector contents first;
/// see [`validate_mv3`] and [`Mv3WriteError`] for the specific checks.
pub fn write_mv3(writer: &mut (impl Write + Seek), file: &Mv3File) -> Result<(), Mv3WriteError> {
    validate_mv3(file)?;
    file.write(writer)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn sample_mv3() -> Mv3File {
        Mv3File {
            version: 3,
            duration: 1000,
            texture_count: 1,
            unknown_data_count: 1,
            model_count: 1,
            action_count: 1,
            action_desc: vec![Mv3ActionDesc {
                tick: 30,
                name: "walk".into(),
            }],
            unknown_data: vec![Mv3UnknownDataInFile {
                unknown0: vec![7u8; 64],
                unknown1: 42,
                unknown2_count: 1,
                unknown2: vec![[1.5f32; 17]],
            }],
            textures: vec![Mv3Texture {
                unknown: vec![0.25f32; 17],
                names: vec![
                    "a.bmp".into(),
                    "b.bmp".into(),
                    "c.bmp".into(),
                    "d.bmp".into(),
                ],
            }],
            models: vec![Mv3Model {
                unknown: vec![9u8; 64],
                vertex_per_frame: 2,
                aabb_min: [-1.0, -2.0, -3.0],
                aabb_max: [1.0, 2.0, 3.0],
                frame_count: 2,
                frames: vec![
                    Mv3Frame {
                        timestamp: 0,
                        vertices: vec![
                            Mv3Vertex {
                                x: 10,
                                y: 20,
                                z: 30,
                                normal_phi: 1,
                                normal_theta: 2,
                            },
                            Mv3Vertex {
                                x: -10,
                                y: -20,
                                z: -30,
                                normal_phi: 3,
                                normal_theta: 4,
                            },
                        ],
                    },
                    Mv3Frame {
                        timestamp: 100,
                        vertices: vec![
                            Mv3Vertex {
                                x: 11,
                                y: 21,
                                z: 31,
                                normal_phi: 5,
                                normal_theta: 6,
                            },
                            Mv3Vertex {
                                x: -11,
                                y: -21,
                                z: -31,
                                normal_phi: 7,
                                normal_theta: 8,
                            },
                        ],
                    },
                ],
                texcoord_count: 1,
                texcoords: vec![TexCoord { u: 0.5, v: 0.75 }],
                mesh_count: 1,
                meshes: vec![Mv3Mesh {
                    unknown: 1,
                    triangle_count: 1,
                    triangles: vec![Mv3Triangle {
                        indices: [0, 1, 0],
                        texcoord_indices: [0, 0, 0],
                    }],
                    unknown_data_count: 1,
                    unknown_data: vec![Mv3UnknownDataInMesh { u: 1, v: 2 }],
                }],
            }],
        }
    }

    #[test]
    fn parse_write_parse_round_trip() {
        let original = sample_mv3();

        let mut buf = Cursor::new(Vec::new());
        write_mv3(&mut buf, &original).expect("write_mv3 should succeed for a valid file");

        buf.set_position(0);
        let roundtripped = read_mv3(&mut buf).expect("re-parsing the written buffer should work");

        // The x/z negation in Mv3Vertex is self-inverse, so a full
        // write-then-read cycle should reproduce the exact in-memory value;
        // comparing the Debug representation exercises every field without
        // requiring every intermediate type to implement PartialEq.
        assert_eq!(format!("{:?}", original), format!("{:?}", roundtripped));
    }

    #[test]
    fn write_rejects_texture_count_mismatch() {
        let mut file = sample_mv3();
        file.texture_count = 2; // doesn't match textures.len() == 1

        let mut buf = Cursor::new(Vec::new());
        let err = write_mv3(&mut buf, &file).unwrap_err();
        assert!(matches!(
            err,
            Mv3WriteError::CountMismatch {
                field: "texture_count",
                expected: 2,
                actual: 1,
            }
        ));
    }

    #[test]
    fn write_rejects_frame_vertex_count_mismatch() {
        let mut file = sample_mv3();
        file.models[0].frames[0].vertices.pop();

        let mut buf = Cursor::new(Vec::new());
        let err = write_mv3(&mut buf, &file).unwrap_err();
        assert!(matches!(
            err,
            Mv3WriteError::FrameVertexCountMismatch {
                model_index: 0,
                frame_index: 0,
                expected: 2,
                actual: 1,
            }
        ));
    }

    #[test]
    fn write_rejects_invalid_aabb() {
        let mut file = sample_mv3();
        file.models[0].aabb_min = [5.0, 0.0, 0.0];
        file.models[0].aabb_max = [-5.0, 0.0, 0.0];

        let mut buf = Cursor::new(Vec::new());
        let err = write_mv3(&mut buf, &file).unwrap_err();
        assert!(matches!(
            err,
            Mv3WriteError::InvalidAabb {
                index: 0,
                axis: 0,
                ..
            }
        ));
    }

    #[test]
    fn write_rejects_action_name_too_long() {
        let mut file = sample_mv3();
        file.action_desc[0].name = "this name is definitely too long for 16 bytes".into();

        let mut buf = Cursor::new(Vec::new());
        let err = write_mv3(&mut buf, &file).unwrap_err();
        assert!(matches!(
            err,
            Mv3WriteError::ActionNameTooLong { index: 0, .. }
        ));
    }
}
