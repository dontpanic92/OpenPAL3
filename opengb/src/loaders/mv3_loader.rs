use crate::utilities::ReadExt;
use byteorder::{LittleEndian, ReadBytesExt};
use mini_fs::{MiniFs, StoreExt};
use serde::Serialize;
use std::io::{BufReader, Read};
use std::path::Path;
use std::{error::Error, io::Cursor};

#[derive(Debug, Serialize)]
pub struct Mv3Texture {
    pub unknown: Vec<u8>, // size: 68
    pub names: Vec<Vec<u8>>,
}

#[derive(Debug, Serialize)]
pub struct Mv3Vertex {
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub normal_phi: i8,
    pub normal_theta: u8,
}

#[derive(Debug, Serialize)]
pub struct Mv3Frame {
    pub timestamp: u32,
    pub vertices: Vec<Mv3Vertex>,
}

#[derive(Debug, Serialize)]
pub struct Mv3Triangle {
    pub indices: [u16; 3],
    pub texcoord_indices: [u16; 3],
}

#[derive(Debug, Serialize)]
pub struct Mv3UnknownDataInMesh {
    pub u: u16,
    pub v: u16,
}

#[derive(Debug, Serialize)]
pub struct Mv3Mesh {
    pub unknown: u32,
    pub triangle_count: u32,
    pub triangles: Vec<Mv3Triangle>,
    pub unknown_data_count: u32,
    pub unknown_data: Vec<Mv3UnknownDataInMesh>,
}

#[derive(Debug, Serialize)]
pub struct Mv3TexCoord {
    pub u: f32,
    pub v: f32,
}

#[derive(Debug, Serialize)]
pub struct Mv3Model {
    pub unknown: Vec<u8>, // size: 64
    pub vertex_per_frame: u32,
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],
    pub frame_count: u32,
    pub frames: Vec<Mv3Frame>,
    pub texcoord_count: u32,
    pub texcoords: Vec<Mv3TexCoord>, // size: 8 * unknown2_data_count
    pub mesh_count: u32,
    pub meshes: Vec<Mv3Mesh>,
}

#[derive(Debug, Serialize)]
pub struct Mv3File {
    pub magic: [u8; 4],
    pub unknown_dw: u32,
    pub unknown_dw2: u32,
    pub texture_count: u32,
    pub unknown_data_count: u32,
    pub model_count: u32,
    pub action_count: u32,
    pub action_desc: Vec<[u8; 20]>,
    pub unknown_data: Vec<Vec<u8>>,
    pub textures: Vec<Mv3Texture>,
    pub models: Vec<Mv3Model>,
}

pub fn mv3_load_from_file<P: AsRef<Path>>(
    vfs: &MiniFs,
    path: P,
) -> Result<Mv3File, Box<dyn Error>> {
    let mut reader = BufReader::new(vfs.open(&path)?);
    mv3_load(&mut reader)
}

pub fn mv3_load_from_data(data: Vec<u8>) -> Result<Mv3File, Box<dyn Error>> {
    let mut reader = BufReader::new(Cursor::new(data));
    mv3_load(&mut reader)
}

pub fn mv3_load<T: Read>(reader: &mut BufReader<T>) -> Result<Mv3File, Box<dyn Error>> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;

    match magic {
        [0x4d, 0x56, 0x33, 0x00] => (), // "MV3\0"
        _ => panic!("Not a valid mv3 file"),
    }

    let unknown_dw = reader.read_u32::<LittleEndian>()?;
    let unknown_dw2 = reader.read_u32::<LittleEndian>()?;
    let texture_count = reader.read_u32::<LittleEndian>()?;
    let unknown_data_count = reader.read_u32::<LittleEndian>()?;
    let model_count = reader.read_u32::<LittleEndian>()?;
    let action_count = reader.read_u32::<LittleEndian>()?;

    let mut action_desc = vec![];
    for _i in 0..action_count {
        let mut buf = [0; 20];
        reader.read_exact(&mut buf)?;
        action_desc.push(buf);
    }

    let unknown_data = vec![];
    for _i in 0..unknown_data_count {
        let _buf = reader.read_u8_vec(64)?;
        reader.read_u32::<LittleEndian>()?;
        let count = reader.read_u32::<LittleEndian>()?;
        for _j in 0..count {
            reader.read_u8_vec(68)?;
        }
    }

    let mut textures = vec![];
    for _i in 0..texture_count {
        let texture = {
            let buf = reader.read_u8_vec(68)?;
            let mut names = vec![];

            for _j in 0..4 {
                let name_length = reader.read_u32::<LittleEndian>()?;

                let name = if name_length > 0 {
                    reader.read_u8_vec(name_length as usize)?
                } else {
                    vec![]
                };

                names.push(name);
            }

            Mv3Texture {
                unknown: buf,
                names,
            }
        };

        textures.push(texture);
    }

    let mut models = vec![];
    for _i in 0..model_count {
        let model = read_mv3_model(reader)?;
        models.push(model);
    }

    Ok(Mv3File {
        magic,
        unknown_dw,
        unknown_dw2,
        texture_count,
        unknown_data_count,
        model_count,
        action_count,
        action_desc,
        unknown_data,
        textures,
        models,
    })
}

fn read_mv3_model(reader: &mut dyn Read) -> Result<Mv3Model, Box<dyn Error>> {
    let unknown = reader.read_u8_vec(64)?;
    let vertex_per_frame = reader.read_u32::<LittleEndian>()?;
    let mut aabb_min = [0f32; 3];
    let mut aabb_max = [0f32; 3];
    reader.read_f32_into::<LittleEndian>(&mut aabb_min)?;
    reader.read_f32_into::<LittleEndian>(&mut aabb_max)?;
    let frame_count = reader.read_u32::<LittleEndian>()?;
    let mut frames = vec![];
    for _i in 0..frame_count {
        let timestamp = reader.read_u32::<LittleEndian>()?;
        let mut vertices = vec![];
        for _j in 0..vertex_per_frame {
            let x = reader.read_i16::<LittleEndian>()?;
            let y = reader.read_i16::<LittleEndian>()?;
            let z = reader.read_i16::<LittleEndian>()?;
            let normal_phi = reader.read_i8()?;
            let normal_theta = reader.read_u8()?;
            vertices.push(Mv3Vertex {
                x: -x,
                y,
                z: -z,
                normal_phi,
                normal_theta,
            });
        }
        frames.push(Mv3Frame {
            timestamp,
            vertices,
        });
    }

    let texcoord_count = reader.read_u32::<LittleEndian>()?;
    let mut texcoords = vec![];

    for _i in 0..texcoord_count {
        let u = reader.read_f32::<LittleEndian>()?;
        let v = reader.read_f32::<LittleEndian>()?;
        texcoords.push(Mv3TexCoord { u, v });
    }

    let mesh_count = reader.read_u32::<LittleEndian>()?;
    let mut meshes = vec![];
    for _i in 0..mesh_count {
        meshes.push(read_mv3_mesh(reader)?);
    }

    Ok(Mv3Model {
        unknown,
        vertex_per_frame,
        aabb_min,
        aabb_max,
        frame_count,
        frames,
        texcoord_count,
        texcoords,
        mesh_count,
        meshes,
    })
}

fn read_mv3_mesh(reader: &mut dyn Read) -> Result<Mv3Mesh, Box<dyn Error>> {
    let unknown = reader.read_u32::<LittleEndian>()?;
    let triangle_count = reader.read_u32::<LittleEndian>()?;
    let mut triangles = vec![];
    for _i in 0..triangle_count {
        let mut indices = [0u16; 3];
        let mut texcoord_indices = [0u16; 3];
        reader.read_u16_into::<LittleEndian>(&mut indices)?;
        reader.read_u16_into::<LittleEndian>(&mut texcoord_indices)?;

        triangles.push(Mv3Triangle {
            indices,
            texcoord_indices,
        })
    }

    let unknown_data_count = reader.read_u32::<LittleEndian>()?;
    let mut unknown_data = vec![];
    for _i in 0..unknown_data_count {
        let u = reader.read_u16::<LittleEndian>()?;
        let v = reader.read_u16::<LittleEndian>()?;
        unknown_data.push(Mv3UnknownDataInMesh { u, v })
    }

    Ok(Mv3Mesh {
        unknown,
        triangle_count,
        triangles,
        unknown_data_count,
        unknown_data,
    })
}
