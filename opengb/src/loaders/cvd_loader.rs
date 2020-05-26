use super::{calc_vertex_size, read_vec};
use byteorder::{LittleEndian, ReadBytesExt};
use encoding::{DecoderTrap, Encoding};
use radiance::math::{Mat44, Quaternion, Vec2, Vec3};
use std::error::Error;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct CvdVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub tex_coord: Vec2,
}

#[derive(Debug)]
pub struct CvdTriangle {
    pub indices: [u16; 3],
}

#[derive(Debug)]
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

#[derive(Debug)]
pub struct CvdMesh {
    pub frame_count: u32,
    pub vertex_count: u32,
    pub frames: Vec<Vec<CvdVertex>>,
    pub unknown_data: Vec<f32>,
    pub material_count: u32,
    pub materials: Vec<CvdMaterial>,
}

#[derive(Debug)]
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

#[derive(Debug)]
pub struct CvdPositionKeyFrames {
    pub version: u8,
    pub frames: Vec<CvdPositionKeyFrame>,
}

#[derive(Debug)]
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

#[derive(Debug)]
pub struct CvdRotationKeyFrames {
    pub version: u8,
    pub frames: Vec<CvdRotationKeyFrame>,
}

#[derive(Debug)]
pub struct CvdScaleKeyFrame {
    pub timestamp: f32,
    pub quaternion: Quaternion,
    pub scale: Vec3,
    pub unknown: [f32; 14],
}

#[derive(Debug)]
pub struct CvdScaleKeyFrames {
    pub version: u8,
    pub frames: Vec<CvdScaleKeyFrame>,
}

#[derive(Debug)]
pub struct CvdModel {
    pub unknown_byte: u8,
    pub scale_factor: f32,
    pub position_keyframes: Option<CvdPositionKeyFrames>,
    pub rotation_keyframes: Option<CvdRotationKeyFrames>,
    pub scale_keyframes: Option<CvdScaleKeyFrames>,
    pub mesh: CvdMesh,
}

#[derive(Debug)]
pub struct CvdModelNode {
    pub model: Option<CvdModel>,
    pub children: Option<Vec<CvdModelNode>>,
}

#[derive(Debug)]
pub struct CvdFile {
    pub magic: [u8; 4],
    pub model_count: u32,
    pub models: Vec<CvdModelNode>,
}

pub fn cvd_load_from_file<P: AsRef<Path>>(path: P) -> Result<CvdFile, Box<dyn Error>> {
    let mut reader = BufReader::new(fs::File::open(&path).unwrap());
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).unwrap();

    let unknown_float = match magic {
        [0x63, 0x76, 0x64, 0x73] => 0.5, // "cvds"
        [0x63, 0x76, 0x64, 0x66] => 0.4, // "cvdf"
        _ => panic!("Not a valid cvd file"),
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
        if model.is_some() {
            models.push(model.unwrap());
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
    let unknown_byte = reader.read_u8().unwrap();

    let mut model = None;
    if unknown_byte > 0 {
        let position_keyframes = read_position_keyframes(reader);
        let rotation_keyframes = read_rotation_keyframes(reader);
        let scale_keyframes = read_scale_keyframes(reader);

        let scale_factor = reader.read_f32::<LittleEndian>().unwrap();
        let mesh = cvd_load_mesh(reader, unknown_float).unwrap();

        let mut mat = Mat44::new_zero();
        reader
            .read_f32_into::<LittleEndian>(unsafe {
                std::mem::transmute::<&mut [[f32; 4]; 4], &mut [f32; 16]>(mat.floats_mut())
            })
            .unwrap();

        model = Some(CvdModel {
            unknown_byte,
            scale_factor,
            position_keyframes,
            rotation_keyframes,
            scale_keyframes,
            mesh,
        });
    }

    let children_count = reader.read_u32::<LittleEndian>().unwrap();
    let mut models = None;
    if children_count > 0 {
        models = Some(vec![]);
        for _i in 0..children_count {
            let model = cvd_load_model(reader, unknown_float).unwrap().unwrap();
            models.as_mut().unwrap().push(model);
        }
    }

    Ok(Some(CvdModelNode {
        model,
        children: models,
    }))
}

pub fn cvd_load_mesh(reader: &mut dyn Read, unknown_float: f32) -> Result<CvdMesh, Box<dyn Error>> {
    let frame_count = reader.read_u32::<LittleEndian>().unwrap();
    let vertex_count = reader.read_u32::<LittleEndian>().unwrap();
    let vertex_size = calc_vertex_size(19);
    let mut frames = vec![];
    for _i in 0..frame_count {
        let mut vertices = vec![];
        for _j in 0..vertex_count {
            let tx = reader.read_f32::<LittleEndian>().unwrap();
            let ty = reader.read_f32::<LittleEndian>().unwrap();
            let nx = reader.read_f32::<LittleEndian>().unwrap();
            let ny = reader.read_f32::<LittleEndian>().unwrap();
            let nz = reader.read_f32::<LittleEndian>().unwrap();
            let px = reader.read_f32::<LittleEndian>().unwrap();
            let py = reader.read_f32::<LittleEndian>().unwrap();
            let pz = reader.read_f32::<LittleEndian>().unwrap();
            vertices.push(CvdVertex {
                position: Vec3::new(px, pz, -py),
                normal: Vec3::new(nx, ny, nz),
                tex_coord: Vec2::new(tx, ty),
            })
        }

        frames.push(vertices);
    }

    let mut unknown_data = vec![0f32; frame_count as usize];
    reader
        .read_f32_into::<LittleEndian>(unknown_data.as_mut_slice())
        .unwrap();

    let material_count = reader.read_u32::<LittleEndian>().unwrap();
    let mut materials = vec![];
    for _i in 0..material_count {
        let unknown_byte = reader.read_u8().unwrap();
        let color1 = reader.read_u32::<LittleEndian>().unwrap();
        let color2 = reader.read_u32::<LittleEndian>().unwrap();
        let color3 = reader.read_u32::<LittleEndian>().unwrap();
        let color4 = reader.read_u32::<LittleEndian>().unwrap();
        let unknown_float2 = reader.read_f32::<LittleEndian>().unwrap();
        let name = read_vec(reader, 64).unwrap();
        let texture_name = encoding::all::GBK
            .decode(
                &name
                    .into_iter()
                    .take_while(|&c| c != 0)
                    .collect::<Vec<u8>>(),
                DecoderTrap::Ignore,
            )
            .unwrap();

        let triangle_count = reader.read_u32::<LittleEndian>().unwrap();
        let mut triangles = None;
        if triangle_count > 0 {
            triangles = Some(vec![]);
            for _j in 0..triangle_count {
                let index1 = reader.read_u16::<LittleEndian>().unwrap();
                let index2 = reader.read_u16::<LittleEndian>().unwrap();
                let index3 = reader.read_u16::<LittleEndian>().unwrap();
                triangles.as_mut().unwrap().push(CvdTriangle {
                    indices: [index1, index2, index3],
                })
            }
        }

        if unknown_float >= 0.5 {
            let unknown_data2_count = reader.read_u32::<LittleEndian>().unwrap();
            if unknown_data2_count > 0 {
                for _k in 0..unknown_data2_count {
                    let _ = reader.read_u32::<LittleEndian>().unwrap();
                }

                for _k in 0..unknown_data2_count {
                    let _ = read_vec(reader, 20);
                }
            }
        }

        materials.push(CvdMaterial {
            unknown_byte,
            color1,
            color2,
            color3,
            color4,
            texture_name,
            triangle_count,
            triangles,
        });
    }

    Ok(CvdMesh {
        frame_count,
        vertex_count,
        frames,
        unknown_data,
        material_count,
        materials,
    })
}

fn read_position_keyframes(reader: &mut dyn Read) -> Option<CvdPositionKeyFrames> {
    let count = reader.read_i32::<LittleEndian>().unwrap();
    if count <= 0 {
        return None;
    }

    let version = reader.read_u8().unwrap();
    let mut frames = vec![];
    for _i in 0..count {
        let timestamp = reader.read_f32::<LittleEndian>().unwrap();
        let unknown1 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown2 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown3 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown4 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown5 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown6 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown7 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown8 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown9 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown10 = reader.read_f32::<LittleEndian>().unwrap();

        let mut position;
        match version {
            1 => position = Vec3::new(unknown7, unknown8, unknown9),
            2 => position = Vec3::new(unknown8, unknown9, unknown10),
            3 => position = Vec3::new(unknown2, unknown3, unknown4),
            _ => panic!("Unsupported position key frames version: {}", version),
        }

        std::mem::swap(&mut position.y, &mut position.z);
        position.z = -position.z;

        frames.push(CvdPositionKeyFrame {
            timestamp,
            position,
            unknown1,
            unknown2,
            unknown3,
            unknown4,
            unknown5,
            unknown6,
            unknown7,
            unknown8,
            unknown9,
            unknown10,
        })
    }

    Some(CvdPositionKeyFrames { version, frames })
}

fn read_rotation_keyframes(reader: &mut dyn Read) -> Option<CvdRotationKeyFrames> {
    let count = reader.read_i32::<LittleEndian>().unwrap();
    if count <= 0 {
        return None;
    }

    let version = reader.read_u8().unwrap();
    let mut frames = vec![];
    for _i in 0..count {
        let timestamp = reader.read_f32::<LittleEndian>().unwrap();
        let unknown1 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown2 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown3 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown4 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown5 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown6 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown7 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown8 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown9 = reader.read_f32::<LittleEndian>().unwrap();
        let unknown10 = reader.read_f32::<LittleEndian>().unwrap();

        let mut quaternion;
        match version {
            1 => {
                quaternion =
                    Quaternion::from_axis_angle(&Vec3::new(unknown7, unknown8, unknown9), unknown10)
            }
            2 | 3 => quaternion = Quaternion::new(unknown2, unknown3, unknown4, unknown5),
            _ => panic!("Unsupported position key frames version: {}", version),
        }

        std::mem::swap(&mut quaternion.y, &mut quaternion.z);
        quaternion.z = -quaternion.z;

        frames.push(CvdRotationKeyFrame {
            timestamp,
            quaternion,
            unknown1,
            unknown2,
            unknown3,
            unknown4,
            unknown5,
            unknown6,
            unknown7,
            unknown8,
            unknown9,
            unknown10,
        })
    }

    Some(CvdRotationKeyFrames { version, frames })
}

fn read_scale_keyframes(reader: &mut dyn Read) -> Option<CvdScaleKeyFrames> {
    let count = reader.read_i32::<LittleEndian>().unwrap();
    if count <= 0 {
        return None;
    }

    let version = reader.read_u8().unwrap();
    let mut frames = vec![];
    for _i in 0..count {
        let timestamp = reader.read_f32::<LittleEndian>().unwrap();
        let mut unknown = [0f32; 14];
        reader.read_f32_into::<LittleEndian>(&mut unknown).unwrap();

        let mut quaternion;
        let mut scale;
        match version {
            1 => {
                quaternion = Quaternion::new(unknown[9], unknown[10], unknown[11], unknown[12]);
                scale = Vec3::new(unknown[6], unknown[7], unknown[8]);
            }
            2 => {
                quaternion = Quaternion::new(unknown[10], unknown[11], unknown[12], unknown[13]);
                scale = Vec3::new(unknown[7], unknown[8], unknown[9]);
            }
            3 => {
                quaternion = Quaternion::new(unknown[4], unknown[5], unknown[6], unknown[7]);
                scale = Vec3::new(unknown[1], unknown[2], unknown[3]);
            }
            _ => panic!("Unsupported position key frames version: {}", version),
        }

        std::mem::swap(&mut quaternion.y, &mut quaternion.z);
        quaternion.z = -quaternion.z;
        std::mem::swap(&mut scale.y, &mut scale.z);
        // scale.z = -scale.z;

        frames.push(CvdScaleKeyFrame {
            timestamp,
            quaternion,
            scale,
            unknown,
        })
    }

    Some(CvdScaleKeyFrames { version, frames })
}
