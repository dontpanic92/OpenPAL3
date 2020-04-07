use super::{read_w_vec, read_vec, read_f32_vec};
use byteorder::{LittleEndian, ReadBytesExt};
use encoding::{DecoderTrap, Encoding};
use radiance::math::Mat44;
use std::error::Error;
use std::fs;
use std::io::{BufReader, Read};
use std::path::Path;

#[derive(Debug)]
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

#[derive(Debug)]
pub struct PolVertexPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug)]
pub struct PolVertexTexCoord {
    pub u: f32,
    pub v: f32,
}

#[derive(Debug)]
pub struct PolVertex {
    pub position: PolVertexPosition,
    pub normal: Option<[f32; 3]>,
    pub unknown4: Option<[f32; 1]>,
    pub unknown8: Option<[f32; 1]>,
    pub tex_coord: PolVertexTexCoord,
    pub tex_coord2: Option<PolVertexTexCoord>,
    pub unknown40: Option<[f32; 2]>,
    pub unknown80: Option<[f32; 2]>,
    pub unknown100: Option<[f32; 4]>,
}

#[derive(Debug)]
pub struct PolMaterialInfo {
    pub has_alpha: u32,
    pub unknown_68: Vec<f32>,
    pub unknown_float: f32,
    pub texture_count: u32,
    pub texture_names: Vec<String>,
    pub unknown2: u32,
    pub unknown3: u32,
    pub unknown4: u32,
    pub triangle_count: u32,
    pub triangles: Vec<PolTriangle>,
}

#[derive(Debug)]
pub struct PolTriangle {
    pub indices: [u16; 3],
}

#[derive(Debug)]
pub struct PolMesh {
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],
    pub vertex_type: PolVertexComponents,
    pub vertex_count: u32,
    pub vertices: Vec<PolVertex>,
    pub material_info_count: u32,
    pub material_info: Vec<PolMaterialInfo>,
}

#[derive(Debug)]
pub struct UnknownData {
    pub unknown: Vec<u8>, // size: 32
    pub matrix: Mat44,
    pub unknown2: u32,
    pub str_len: u32,
    pub ddd_str: Vec<u8>,
}

#[derive(Debug)]
pub struct GeomNodeDesc {
    pub unknown: Vec<u16>, // size: 52
}

#[derive(Debug)]
pub struct PolFile {
    pub magic: [u8; 4],
    pub some_flag: u32,
    pub mesh_count: u32,
    pub geom_node_descs: Vec<GeomNodeDesc>,
    pub unknown_count: u32,
    pub unknown_data: Vec<UnknownData>,
    pub meshes: Vec<PolMesh>,
}

pub fn pol_load_from_file<P: AsRef<Path>>(path: P) -> Result<PolFile, Box<dyn Error>> {
    let mut reader = BufReader::new(fs::File::open(path)?);
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;

    match magic {
        [0x50, 0x4f, 0x4c, 0x59] => (), // "POLY"
        _ => panic!("Not a valid pol file"),
    }

    let some_flag = reader.read_u32::<LittleEndian>()?;
    let mesh_count = reader.read_u32::<LittleEndian>()?;
    let mut geom_node_descs = vec![];
    for _i in 0..mesh_count {
        let unknown = read_w_vec(&mut reader, 26)?;
        println!("geom node desc {:?}", &unknown);
        geom_node_descs.push(GeomNodeDesc { unknown });
    }

    let mut unknown_count = 0;
    let mut unknown_data = vec![];
    if some_flag > 100 {
        unknown_count = reader.read_u32::<LittleEndian>()?;
        if unknown_count > 0 {
            for _i in 0..unknown_count {
                let u = read_vec(&mut reader, 32)?;
                let mut mat = Mat44::new_zero();
                reader.read_f32_into::<LittleEndian>(unsafe {
                    std::mem::transmute::<&mut [[f32; 4]; 4], &mut [f32; 16]>(mat.floats_mut())
                })?;
                let u2 = reader.read_u32::<LittleEndian>()?;
                let str_len = reader.read_u32::<LittleEndian>()?;
                let ddd_str = read_vec(&mut reader, str_len as usize)?;
                unknown_data.push(UnknownData {
                    unknown: u,
                    matrix: mat,
                    unknown2: u2,
                    ddd_str,
                    str_len,
                })
            }
        }
    }

    let mut meshes = vec![];
    for _i in 0..mesh_count {
        meshes.push(read_pol_mesh(&mut reader)?);
    }

    Ok(PolFile {
        magic,
        some_flag,
        mesh_count,
        geom_node_descs,
        unknown_count,
        unknown_data,
        meshes,
    })
}

fn read_pol_mesh(reader: &mut dyn Read) -> Result<PolMesh, Box<dyn Error>> {
    let mut aabb_min = [0f32; 3];
    let mut aabb_max = [0f32; 3];
    reader.read_f32_into::<LittleEndian>(&mut aabb_min)?;
    reader.read_f32_into::<LittleEndian>(&mut aabb_max)?;
    let vertex_type = PolVertexComponents {
        0: reader.read_i32::<LittleEndian>()? as u32,
    };
    let vertex_count = reader.read_u32::<LittleEndian>()?;
    let _size = super::calc_vertex_size(vertex_type.0 as i32);
    let mut vertices = vec![];
    for _i in 0..vertex_count {
        if !vertex_type.has(PolVertexComponents::POSITION) {
            panic!("This POL file doesn't have position info, which doesn't support currently.");
        }

        if !vertex_type.has(PolVertexComponents::TEXCOORD) {
            panic!(
                "This POL file doesn't have texture coord info, which doesn't support currently."
            );
        }

        let position = PolVertexPosition {
            x: reader.read_f32::<LittleEndian>()?,
            y: reader.read_f32::<LittleEndian>()?,
            z: reader.read_f32::<LittleEndian>()?,
        };

        let normal = if vertex_type.has(PolVertexComponents::NORMAL) {
            let mut arr = [0.; 3];
            reader.read_f32_into::<LittleEndian>(&mut arr).unwrap();
            Some(arr)
        } else {
            None
        };

        let unknown4 = if vertex_type.has(PolVertexComponents::UNKNOWN4) {
            let mut arr = [0.; 1];
            reader.read_f32_into::<LittleEndian>(&mut arr)?;
            Some(arr)
        } else {
            None
        };

        let unknown8 = if vertex_type.has(PolVertexComponents::UNKNOWN8) {
            let mut arr = [0.; 1];
            reader.read_f32_into::<LittleEndian>(&mut arr)?;
            Some(arr)
        } else {
            None
        };

        let tex_coord = PolVertexTexCoord {
            u: reader.read_f32::<LittleEndian>()?,
            v: reader.read_f32::<LittleEndian>()?,
        };

        let tex_coord2 = if vertex_type.has(PolVertexComponents::TEXCOORD2) {
            Some(PolVertexTexCoord {
                u: reader.read_f32::<LittleEndian>()?,
                v: reader.read_f32::<LittleEndian>()?,
            })
        } else {
            None
        };

        let unknown40 = if vertex_type.has(PolVertexComponents::UNKNOWN40) {
            let mut arr = [0.; 2];
            reader.read_f32_into::<LittleEndian>(&mut arr)?;
            Some(arr)
        } else {
            None
        };

        let unknown80 = if vertex_type.has(PolVertexComponents::UNKNOWN80) {
            let mut arr = [0.; 2];
            reader.read_f32_into::<LittleEndian>(&mut arr)?;
            Some(arr)
        } else {
            None
        };

        let unknown100 = if vertex_type.has(PolVertexComponents::UNKNOWN100) {
            let mut arr = [0.; 4];
            reader.read_f32_into::<LittleEndian>(&mut arr)?;
            Some(arr)
        } else {
            None
        };

        vertices.push(PolVertex {
            position,
            normal,
            unknown4,
            unknown8,
            tex_coord,
            tex_coord2,
            unknown40,
            unknown80,
            unknown100,
        });
    }

    let material_info_count = reader.read_u32::<LittleEndian>()?;
    let mut material_info = vec![];
    for _i in 0..material_info_count {
        let has_alpha = reader.read_u32::<LittleEndian>()?;
        let unknown_68 = read_f32_vec(reader, 16)?;
        let unknown_float = reader.read_f32::<LittleEndian>()?.min(128.).max(0.);
        let texture_count = reader.read_u32::<LittleEndian>()?;
        let mut texture_names = vec![];
        for _j in 0..texture_count {
            let name = read_vec(reader, 64).unwrap();
            let name_s = encoding::all::GBK
                .decode(
                    &name
                        .into_iter()
                        .take_while(|&c| c != 0)
                        .collect::<Vec<u8>>(),
                    DecoderTrap::Ignore,
                )
                .unwrap();
            texture_names.push(name_s);
        }

        let unknown2 = reader.read_u32::<LittleEndian>()?;
        let unknown3 = reader.read_u32::<LittleEndian>()?;
        let unknown4 = reader.read_u32::<LittleEndian>()?;
        let triangle_count = reader.read_u32::<LittleEndian>()?;
        let mut triangles = vec![];
        for _i in 0..triangle_count {
            let mut indices = [0u16; 3];
            reader.read_u16_into::<LittleEndian>(&mut indices)?;
            triangles.push(PolTriangle { indices })
        }

        println!("unknown_dw0 {} unknown_68 {:?} unknown_float {} unknown2 {} unknown3 {} unknown4 {}", has_alpha, unknown_68, unknown_float, unknown2, unknown3, unknown4);

        material_info.push(PolMaterialInfo {
            has_alpha,
            unknown_68,
            unknown_float,
            texture_count,
            texture_names,
            unknown2,
            unknown3,
            unknown4,
            triangle_count,
            triangles,
        });
    }

    Ok(PolMesh {
        aabb_min,
        aabb_max,
        vertex_type,
        vertex_count,
        vertices,
        material_info_count,
        material_info,
    })
}
