use binrw::{BinRead, BinWrite};
use serde::Serialize;
use std::io::{Read, Seek, Write};
use thiserror::Error;

use crate::{
    rwbs::{Matrix44f, TexCoord, Vec3f},
    utils::{SizedString, StringWithCapacity},
};

#[derive(BinRead, BinWrite, Debug, Serialize, Clone, Copy)]
#[brw(little)]
pub struct PolVertexComponents(u32);
impl PolVertexComponents {
    pub const POSITION: Self = PolVertexComponents(0b1);
    pub const NORMAL: Self = PolVertexComponents(0b10);
    pub const UNKNOWN4: Self = PolVertexComponents(0b100);
    pub const UNKNOWN8: Self = PolVertexComponents(0b1000);
    pub const TEXCOORD: Self = PolVertexComponents(0b10000);
    pub const TEXCOORD2: Self = PolVertexComponents(0b100000);
    pub const UNKNOWN40: Self = PolVertexComponents(0b1000000);
    pub const UNKNOWN80: Self = PolVertexComponents(0b10000000);
    pub const UNKNOWN100: Self = PolVertexComponents(0b100000000);

    pub fn has(&self, c: PolVertexComponents) -> bool {
        (self.0 & c.0) != 0
    }
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little)]
pub struct PolVertexPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little, import(t: PolVertexComponents))]
pub struct PolVertex {
    #[brw(args{half_float: false})]
    pub position: Vec3f,
    #[br(if(t.has(PolVertexComponents::NORMAL)))]
    #[brw(args{half_float: false})]
    pub normal: Option<Vec3f>,
    #[br(if(t.has(PolVertexComponents::UNKNOWN4)))]
    pub unknown4: Option<f32>,
    #[br(if(t.has(PolVertexComponents::UNKNOWN8)))]
    pub unknown8: Option<f32>,
    pub tex_coord: TexCoord,
    #[br(if(t.has(PolVertexComponents::TEXCOORD2)))]
    pub tex_coord2: Option<TexCoord>,
    #[br(if(t.has(PolVertexComponents::UNKNOWN40)))]
    pub unknown40: Option<[f32; 2]>,
    #[br(if(t.has(PolVertexComponents::UNKNOWN80)))]
    pub unknown80: Option<[f32; 2]>,
    #[br(if(t.has(PolVertexComponents::UNKNOWN100)))]
    pub unknown100: Option<[f32; 4]>,
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little)]
pub struct PolMaterialInfo {
    pub use_alpha: u32,
    #[br(count = 16)]
    pub unknown_68: Vec<f32>,
    pub unknown_float: f32,
    pub texture_count: u32,
    #[br(count = texture_count, args { inner: (64,) })]
    #[bw(args(64))]
    pub texture_names: Vec<StringWithCapacity>,
    pub unknown2: u32,
    pub unknown3: u32,
    pub unknown4: u32,
    pub triangle_count: u32,
    #[br(count = triangle_count)]
    pub triangles: Vec<PolTriangle>,
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little)]
pub struct PolTriangle {
    pub indices: [u16; 3],
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(
    little,
    assert(
        vertex_type.has(PolVertexComponents::POSITION)
        && vertex_type.has(PolVertexComponents::TEXCOORD),
        "This POL file doesn't have POSITION or TEXCOORD info, which doesn't support currently."
    )
)]
pub struct PolMesh {
    #[brw(args{half_float: false})]
    pub aabb_min: Vec3f,
    #[brw(args{half_float: false})]
    pub aabb_max: Vec3f,
    pub vertex_type: PolVertexComponents,
    pub vertex_count: u32,
    #[br(count = vertex_count as usize, args { inner: (vertex_type,) })]
    #[bw(args(*vertex_type))]
    pub vertices: Vec<PolVertex>,
    pub material_info_count: u32,
    #[br(count = material_info_count)]
    pub material_info: Vec<PolMaterialInfo>,
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little)]
pub struct UnknownData {
    #[br(count = 32)]
    pub unknown: Vec<u8>, // size: 32
    pub matrix: Matrix44f,
    pub unknown2: u32,
    pub ddd_str: SizedString,
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little)]
pub struct GeomNodeDesc {
    #[br(count = 26)]
    pub unknown: Vec<u16>, // size: 52
}

#[derive(BinRead, BinWrite, Debug, Serialize, Clone)]
#[brw(little, magic = b"POLY")]
pub struct PolFile {
    pub some_flag: u32,
    pub mesh_count: u32,
    #[br(count = mesh_count)]
    pub geom_node_descs: Vec<GeomNodeDesc>,
    #[br(if(some_flag > 100))]
    #[bw(if(*some_flag > 100))]
    pub unknown_count: u32,
    #[br(if(some_flag > 100), count = unknown_count)]
    #[bw(if(*some_flag > 100))]
    pub unknown_data: Vec<UnknownData>,
    #[br(count = mesh_count)]
    pub meshes: Vec<PolMesh>,
}

pub fn read_pol(reader: &mut (impl Read + Seek)) -> anyhow::Result<PolFile> {
    Ok(PolFile::read(reader)?)
}

/// Errors produced while validating a [`PolFile`] before it is serialized.
///
/// Like MV3, POL stores explicit element counts next to the vectors they
/// describe (`mesh_count`, `vertex_count`, `texture_count`, ...), some of
/// which are conditionally present (`unknown_count`/`unknown_data` only
/// exist when `some_flag > 100`). [`write_pol`] checks all of these
/// invariants, plus the AABB bounds and the vertex component flags, before
/// serializing so a malformed in-memory tree is rejected with a precise
/// error instead of silently producing a corrupt file.
#[derive(Debug, Error)]
pub enum PolWriteError {
    #[error(
        "POL {field} count mismatch: header/count field says {expected}, but {actual} elements are present"
    )]
    CountMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },

    #[error("POL geom_node_desc #{index} unknown must be exactly 26 u16s, got {actual}")]
    InvalidGeomNodeDescLen { index: usize, actual: usize },

    #[error(
        "POL mesh #{index} has an invalid AABB: min ({min:?}) is greater than max ({max:?}) on axis {axis}"
    )]
    InvalidAabb {
        index: usize,
        axis: usize,
        min: f32,
        max: f32,
    },

    #[error(
        "POL mesh #{index} is missing POSITION and/or TEXCOORD in vertex_type, which this writer (and the existing reader) doesn't support"
    )]
    MissingRequiredVertexComponents { index: usize },

    #[error(
        "POL mesh #{mesh_index} vertex #{vertex_index} component {component} presence doesn't match vertex_type flags"
    )]
    VertexComponentMismatch {
        mesh_index: usize,
        vertex_index: usize,
        component: &'static str,
    },

    #[error("POL material_info #{index} unknown_68 must be exactly 16 floats, got {actual}")]
    InvalidMaterialUnknown68Len { index: usize, actual: usize },

    #[error("failed to serialize POL file: {0}")]
    Binrw(#[from] binrw::Error),
}

fn check_count(field: &'static str, expected: u32, actual: usize) -> Result<(), PolWriteError> {
    if expected as usize != actual {
        return Err(PolWriteError::CountMismatch {
            field,
            expected: expected as usize,
            actual,
        });
    }
    Ok(())
}

fn check_component<T>(
    mesh_index: usize,
    vertex_index: usize,
    component: &'static str,
    flag_set: bool,
    value: &Option<T>,
) -> Result<(), PolWriteError> {
    if flag_set != value.is_some() {
        return Err(PolWriteError::VertexComponentMismatch {
            mesh_index,
            vertex_index,
            component,
        });
    }
    Ok(())
}

/// Validate a [`PolFile`]'s internal counts, bounds, and vertex component
/// flags without writing it.
pub fn validate_pol(file: &PolFile) -> Result<(), PolWriteError> {
    check_count("mesh_count", file.mesh_count, file.geom_node_descs.len())?;
    check_count("mesh_count", file.mesh_count, file.meshes.len())?;

    for (index, desc) in file.geom_node_descs.iter().enumerate() {
        if desc.unknown.len() != 26 {
            return Err(PolWriteError::InvalidGeomNodeDescLen {
                index,
                actual: desc.unknown.len(),
            });
        }
    }

    if file.some_flag > 100 {
        check_count("unknown_count", file.unknown_count, file.unknown_data.len())?;
    } else if !file.unknown_data.is_empty() || file.unknown_count != 0 {
        return Err(PolWriteError::CountMismatch {
            field: "unknown_count",
            expected: 0,
            actual: file.unknown_data.len().max(file.unknown_count as usize),
        });
    }

    for (index, mesh) in file.meshes.iter().enumerate() {
        if !mesh.vertex_type.has(PolVertexComponents::POSITION)
            || !mesh.vertex_type.has(PolVertexComponents::TEXCOORD)
        {
            return Err(PolWriteError::MissingRequiredVertexComponents { index });
        }

        for axis in 0..3 {
            let (min, max) = (
                [mesh.aabb_min.x, mesh.aabb_min.y, mesh.aabb_min.z][axis],
                [mesh.aabb_max.x, mesh.aabb_max.y, mesh.aabb_max.z][axis],
            );
            if min > max {
                return Err(PolWriteError::InvalidAabb {
                    index,
                    axis,
                    min,
                    max,
                });
            }
        }

        check_count("vertex_count", mesh.vertex_count, mesh.vertices.len())?;
        for (vertex_index, vertex) in mesh.vertices.iter().enumerate() {
            check_component(
                index,
                vertex_index,
                "normal",
                mesh.vertex_type.has(PolVertexComponents::NORMAL),
                &vertex.normal,
            )?;
            check_component(
                index,
                vertex_index,
                "unknown4",
                mesh.vertex_type.has(PolVertexComponents::UNKNOWN4),
                &vertex.unknown4,
            )?;
            check_component(
                index,
                vertex_index,
                "unknown8",
                mesh.vertex_type.has(PolVertexComponents::UNKNOWN8),
                &vertex.unknown8,
            )?;
            check_component(
                index,
                vertex_index,
                "tex_coord2",
                mesh.vertex_type.has(PolVertexComponents::TEXCOORD2),
                &vertex.tex_coord2,
            )?;
            check_component(
                index,
                vertex_index,
                "unknown40",
                mesh.vertex_type.has(PolVertexComponents::UNKNOWN40),
                &vertex.unknown40,
            )?;
            check_component(
                index,
                vertex_index,
                "unknown80",
                mesh.vertex_type.has(PolVertexComponents::UNKNOWN80),
                &vertex.unknown80,
            )?;
            check_component(
                index,
                vertex_index,
                "unknown100",
                mesh.vertex_type.has(PolVertexComponents::UNKNOWN100),
                &vertex.unknown100,
            )?;
        }

        check_count(
            "material_info_count",
            mesh.material_info_count,
            mesh.material_info.len(),
        )?;

        for material in mesh.material_info.iter() {
            if material.unknown_68.len() != 16 {
                return Err(PolWriteError::InvalidMaterialUnknown68Len {
                    index,
                    actual: material.unknown_68.len(),
                });
            }

            check_count(
                "texture_count",
                material.texture_count,
                material.texture_names.len(),
            )?;
            check_count(
                "triangle_count",
                material.triangle_count,
                material.triangles.len(),
            )?;
        }
    }

    Ok(())
}

/// Serialize a [`PolFile`] to `writer`, matching the exact binary layout
/// produced by [`read_pol`].
///
/// All the count fields embedded in the format, the AABB bounds, and the
/// per-vertex optional component flags are validated against the actual
/// vector contents first; see [`validate_pol`] and [`PolWriteError`] for the
/// specific checks.
pub fn write_pol(writer: &mut (impl Write + Seek), file: &PolFile) -> Result<(), PolWriteError> {
    validate_pol(file)?;
    file.write(writer)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn sample_pol() -> PolFile {
        let vertex_type = PolVertexComponents(
            PolVertexComponents::POSITION.0
                | PolVertexComponents::NORMAL.0
                | PolVertexComponents::TEXCOORD.0,
        );

        let vertex = |x: f32, y: f32, z: f32| PolVertex {
            position: Vec3f { x, y, z },
            normal: Some(Vec3f {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            }),
            unknown4: None,
            unknown8: None,
            tex_coord: TexCoord { u: 0.0, v: 0.0 },
            tex_coord2: None,
            unknown40: None,
            unknown80: None,
            unknown100: None,
        };

        PolFile {
            some_flag: 1,
            mesh_count: 1,
            geom_node_descs: vec![GeomNodeDesc {
                unknown: vec![3u16; 26],
            }],
            unknown_count: 0,
            unknown_data: vec![],
            meshes: vec![PolMesh {
                aabb_min: Vec3f {
                    x: -1.0,
                    y: -1.0,
                    z: -1.0,
                },
                aabb_max: Vec3f {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
                vertex_type,
                vertex_count: 3,
                vertices: vec![
                    vertex(0.0, 0.0, 0.0),
                    vertex(1.0, 0.0, 0.0),
                    vertex(0.0, 1.0, 0.0),
                ],
                material_info_count: 1,
                material_info: vec![PolMaterialInfo {
                    use_alpha: 0,
                    unknown_68: vec![0.0f32; 16],
                    unknown_float: 1.0,
                    texture_count: 1,
                    texture_names: vec!["tex.bmp".into()],
                    unknown2: 0,
                    unknown3: 0,
                    unknown4: 0,
                    triangle_count: 1,
                    triangles: vec![PolTriangle { indices: [0, 1, 2] }],
                }],
            }],
        }
    }

    #[test]
    fn parse_write_parse_round_trip() {
        let original = sample_pol();

        let mut buf = Cursor::new(Vec::new());
        write_pol(&mut buf, &original).expect("write_pol should succeed for a valid file");

        buf.set_position(0);
        let roundtripped = read_pol(&mut buf).expect("re-parsing the written buffer should work");

        assert_eq!(format!("{:?}", original), format!("{:?}", roundtripped));
    }

    #[test]
    fn parse_write_parse_round_trip_with_extra_flag_data() {
        let mut original = sample_pol();
        original.some_flag = 101; // enables the unknown_count/unknown_data section
        original.unknown_count = 1;
        original.unknown_data = vec![UnknownData {
            unknown: vec![0u8; 32],
            matrix: Matrix44f([0f32; 16]),
            unknown2: 7,
            ddd_str: "hello".into(),
        }];

        let mut buf = Cursor::new(Vec::new());
        write_pol(&mut buf, &original).expect("write_pol should succeed for a valid file");

        buf.set_position(0);
        let roundtripped = read_pol(&mut buf).expect("re-parsing the written buffer should work");

        assert_eq!(format!("{:?}", original), format!("{:?}", roundtripped));
    }

    #[test]
    fn write_rejects_mesh_count_mismatch() {
        let mut file = sample_pol();
        file.mesh_count = 2;

        let mut buf = Cursor::new(Vec::new());
        let err = write_pol(&mut buf, &file).unwrap_err();
        assert!(matches!(
            err,
            PolWriteError::CountMismatch {
                field: "mesh_count",
                expected: 2,
                actual: 1,
            }
        ));
    }

    #[test]
    fn write_rejects_missing_position_or_texcoord() {
        let mut file = sample_pol();
        file.meshes[0].vertex_type = PolVertexComponents(PolVertexComponents::NORMAL.0);

        let mut buf = Cursor::new(Vec::new());
        let err = write_pol(&mut buf, &file).unwrap_err();
        assert!(matches!(
            err,
            PolWriteError::MissingRequiredVertexComponents { index: 0 }
        ));
    }

    #[test]
    fn write_rejects_vertex_component_flag_mismatch() {
        let mut file = sample_pol();
        // vertex_type declares NORMAL, but clear one vertex's normal.
        file.meshes[0].vertices[0].normal = None;

        let mut buf = Cursor::new(Vec::new());
        let err = write_pol(&mut buf, &file).unwrap_err();
        assert!(matches!(
            err,
            PolWriteError::VertexComponentMismatch {
                mesh_index: 0,
                vertex_index: 0,
                component: "normal",
            }
        ));
    }

    #[test]
    fn write_rejects_invalid_aabb() {
        let mut file = sample_pol();
        file.meshes[0].aabb_min.x = 5.0;
        file.meshes[0].aabb_max.x = -5.0;

        let mut buf = Cursor::new(Vec::new());
        let err = write_pol(&mut buf, &file).unwrap_err();
        assert!(matches!(
            err,
            PolWriteError::InvalidAabb {
                index: 0,
                axis: 0,
                ..
            }
        ));
    }
}
