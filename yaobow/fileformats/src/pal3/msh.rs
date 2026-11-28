//! Original gbengine v100 MSH meshes, MOV (`anm`) motion and MTL materials.
//! Readers: 0x10007da0/0x10006fb0/0x10006090, 0x10007600/0x10007330,
//! and 0x10008140 in the local original gbengine.dll.

use std::collections::HashSet;
use std::io::{self, ErrorKind};

#[derive(Debug, Clone)]
pub struct Mesh {
    pub nodes: Vec<Node>,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub inverse_bind: [f32; 16],
    pub bone: i32,
    pub parent_bone: i32,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub material: usize,
    pub vertices: Vec<Vertex>,
    pub faces: Vec<Face>,
}

#[derive(Debug, Clone)]
pub struct Vertex {
    pub position: [f32; 3],
    pub influences: Vec<(u8, u8)>,
}

#[derive(Debug, Clone)]
pub struct Face {
    pub indices: [u32; 3],
    pub normals: [[f32; 3]; 3],
    pub uv: [[f32; 2]; 3],
}

#[derive(Debug, Clone)]
pub struct Motion {
    pub tracks: Vec<Track>,
    pub events: Vec<(u32, String)>,
    pub duration_ms: u32,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub name: String,
    pub frames: Vec<KeyFrame>,
}

#[derive(Debug, Clone)]
pub struct KeyFrame {
    pub time_ms: u32,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct Material {
    pub diffuse: [f32; 4],
    pub ambient: [f32; 4],
    pub textures: [String; 4],
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message)
}

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> io::Result<&'a [u8]> {
        let value = self.bytes.get(..count).ok_or_else(|| {
            io::Error::new(ErrorKind::UnexpectedEof, "Truncated PAL3 actor asset")
        })?;
        self.bytes = &self.bytes[count..];
        Ok(value)
    }

    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn floats<const N: usize>(&mut self) -> io::Result<[f32; N]> {
        let mut result = [0.; N];
        for value in &mut result {
            *value = f32::from_bits(self.u32()?);
            if !value.is_finite() {
                return Err(invalid("Non-finite PAL3 actor transform"));
            }
        }
        Ok(result)
    }

    fn count(&mut self, minimum_size: usize) -> io::Result<usize> {
        let count = self.u32()? as usize;
        if count > self.bytes.len() / minimum_size {
            return Err(invalid("Invalid PAL3 actor record count"));
        }
        Ok(count)
    }

    fn string(&mut self) -> io::Result<String> {
        let count = self.count(1)?;
        self.string_bytes(count)
    }

    fn string_bytes(&mut self, count: usize) -> io::Result<String> {
        let bytes = self.take(count)?;
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        crate::utils::to_gbk_string(&bytes[..end]).map_err(|_| invalid("Invalid PAL3 actor string"))
    }

    fn header(&mut self, magic: &[u8; 4], size: usize) -> io::Result<()> {
        if self.take(4)? != magic || self.u32()? != 100 {
            return Err(invalid("Unsupported PAL3 actor format"));
        }
        self.take(size - 8)?;
        Ok(())
    }

    fn end(&self) -> io::Result<()> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err(invalid("Trailing PAL3 actor data"))
        }
    }
}

pub fn read_mesh(bytes: &[u8]) -> io::Result<Mesh> {
    let mut reader = Reader { bytes };
    reader.header(b"msh\0", 16)?;
    let roots = reader.count(140)?;
    let mut nodes = vec![];
    for _ in 0..roots {
        read_node(&mut reader, &mut nodes, 0)?;
    }
    let bones: HashSet<_> = nodes
        .iter()
        .filter(|n| n.bone >= 0)
        .map(|n| n.bone)
        .collect();
    if bones.len() != nodes.iter().filter(|n| n.bone >= 0).count()
        || nodes
            .iter()
            .any(|n| n.parent_bone >= 0 && !bones.contains(&n.parent_bone))
    {
        return Err(invalid("Invalid PAL3 skeleton indices"));
    }
    let mut sections = vec![];
    for _ in 0..reader.count(24)? {
        let material = reader.u32()? as usize;
        let vertex_count = reader.count(12)?;
        let face_count = reader.u32()? as usize;
        let edge_count = reader.u32()? as usize;
        reader.u32()?;
        let flags = reader.u32()?;
        if flags & 1 == 0 {
            return Err(invalid("PAL3 mesh has no vertex positions"));
        }
        let mut vertices = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            vertices.push(Vertex {
                position: reader.floats()?,
                influences: vec![],
            });
        }
        if flags & 0x800 != 0 {
            for vertex in &mut vertices {
                let count = reader.take(1)?[0];
                if count > 8 {
                    return Err(invalid("Too many PAL3 skin influences"));
                }
                for _ in 0..count {
                    let pair = reader.take(2)?;
                    if !bones.contains(&(pair[0] as i32)) {
                        return Err(invalid("Unknown PAL3 skin bone"));
                    }
                    vertex.influences.push((pair[0], pair[1]));
                }
            }
        }
        if flags & 0x1000 != 0 {
            reader.take(
                vertex_count
                    .checked_mul(4)
                    .ok_or_else(|| invalid("Mesh overflow"))?,
            )?;
        }
        if face_count > reader.bytes.len() / 68 {
            return Err(invalid("Invalid PAL3 face count"));
        }
        let mut faces = Vec::with_capacity(face_count);
        for _ in 0..face_count {
            let indices = [reader.u32()?, reader.u32()?, reader.u32()?];
            if indices.iter().any(|&index| index as usize >= vertex_count) {
                return Err(invalid("Invalid PAL3 vertex index"));
            }
            let mut normals = [[0.; 3]; 3];
            for normal in &mut normals {
                let packed = u16::from_le_bytes(reader.take(2)?.try_into().unwrap());
                let theta = f64::from(packed as u8) * f64::from(0.012319972_f32);
                let phi = f64::from((packed >> 8) as u8 as i8)
                    * 2.
                    * f64::from(std::f32::consts::PI)
                    * f64::from(1. / 255.0_f32);
                *normal = [
                    (theta.sin() * phi.cos()) as f32,
                    (theta.sin() * phi.sin()) as f32,
                    f64::from(theta as f32).cos() as f32,
                ];
            }
            reader.take(2)?;
            let mut uv = [[0.; 2]; 3];
            // Disk faces contain stale cache words between separate U/V arrays.
            // Only the six values read at native +0x14/+0x2c are coordinates.
            for axis in 0..2 {
                for (corner, coordinate) in uv.iter_mut().enumerate() {
                    if axis != 0 || corner != 0 {
                        reader.take(4)?;
                    }
                    let value = reader.floats::<1>()?[0];
                    coordinate[axis] = (value as f64 * 255.).trunc() as f32 * (1. / 255.);
                }
            }
            reader.take(4)?;
            faces.push(Face {
                indices,
                normals,
                uv,
            });
        }
        reader.take(
            edge_count
                .checked_mul(16)
                .ok_or_else(|| invalid("Mesh overflow"))?,
        )?;
        sections.push(Section {
            material,
            vertices,
            faces,
        });
    }
    reader.end()?;
    Ok(Mesh { nodes, sections })
}

fn read_node(reader: &mut Reader<'_>, nodes: &mut Vec<Node>, depth: usize) -> io::Result<()> {
    if depth > 256 || nodes.len() >= 4096 {
        return Err(invalid("PAL3 skeleton is too deep or large"));
    }
    reader.u32()?;
    let name = reader.string()?;
    let position = reader.floats()?;
    let rotation = reader.floats()?;
    reader.take(40)?;
    let inverse_bind = reader.floats()?;
    let bone = reader.u32()? as i32;
    let parent_bone = reader.u32()? as i32;
    let children = reader.count(140)?;
    nodes.push(Node {
        name,
        position,
        rotation,
        inverse_bind,
        bone,
        parent_bone,
    });
    for _ in 0..children {
        read_node(reader, nodes, depth + 1)?;
    }
    Ok(())
}

fn milliseconds(seconds: f32) -> io::Result<u32> {
    let value = seconds as f64 * 1000.;
    if seconds < 0. || value > u32::MAX as f64 {
        Err(invalid("Invalid PAL3 animation timestamp"))
    } else {
        Ok(value.trunc() as u32)
    }
}

pub fn read_motion(bytes: &[u8]) -> io::Result<Motion> {
    let mut reader = Reader { bytes };
    reader.header(b"anm\0", 12)?;
    let track_count = reader.count(16)?;
    reader.u32()?;
    let event_count = reader.count(20)?;
    let mut events = vec![];
    for _ in 0..event_count {
        let time = milliseconds(reader.floats::<1>()?[0])?;
        events.push((time, reader.string_bytes(16)?));
    }
    let mut tracks = Vec::with_capacity(track_count);
    let mut duration_ms = 0;
    for _ in 0..track_count {
        reader.u32()?;
        let name = reader.string()?;
        let count = reader.count(68)?;
        reader.u32()?;
        if count == 0 {
            return Err(invalid("Empty PAL3 animation track"));
        }
        let mut frames = Vec::with_capacity(count);
        for _ in 0..count {
            let time_ms = milliseconds(reader.floats::<1>()?[0])?;
            if frames
                .last()
                .is_some_and(|last: &KeyFrame| last.time_ms > time_ms)
            {
                return Err(invalid("Unordered PAL3 animation keys"));
            }
            let position = reader.floats()?;
            let rotation = reader.floats()?;
            reader.take(36)?;
            frames.push(KeyFrame {
                time_ms,
                position,
                rotation,
            });
        }
        duration_ms = duration_ms.max(frames.last().unwrap().time_ms);
        tracks.push(Track { name, frames });
    }
    reader.end()?;
    Ok(Motion {
        tracks,
        events,
        duration_ms,
    })
}

pub fn read_materials(bytes: &[u8]) -> io::Result<Vec<Material>> {
    let mut reader = Reader { bytes };
    reader.header(b"mtl\0", 8)?;
    let count = reader.count(84)?;
    let mut materials = Vec::with_capacity(count);
    for _ in 0..count {
        let diffuse = reader.floats()?;
        let ambient = reader.floats()?;
        reader.take(36)?;
        let mut textures = std::array::from_fn(|_| String::new());
        for texture in &mut textures {
            *texture = reader.string()?;
        }
        materials.push(Material {
            diffuse,
            ambient,
            textures,
        });
    }
    reader.end()?;
    Ok(materials)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend(value.to_le_bytes());
    }

    fn floats(bytes: &mut Vec<u8>, values: &[f32]) {
        for value in values {
            word(bytes, value.to_bits());
        }
    }

    fn string(bytes: &mut Vec<u8>, text: &str) {
        word(bytes, text.len() as u32 + 1);
        bytes.extend(text.bytes());
        bytes.push(0);
    }

    fn mesh(bone: u8, last_index: u32) -> Vec<u8> {
        let mut bytes = b"msh\0".to_vec();
        word(&mut bytes, 100);
        word(&mut bytes, 0x7fc00000);
        word(&mut bytes, 0);
        word(&mut bytes, 1);
        word(&mut bytes, 1);
        string(&mut bytes, "Bip01");
        floats(&mut bytes, &[0.; 3]);
        floats(&mut bytes, &[0., 0., 0., 1.]);
        bytes.extend([0xff; 40]);
        floats(
            &mut bytes,
            &[
                1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
            ],
        );
        word(&mut bytes, 0);
        word(&mut bytes, u32::MAX);
        word(&mut bytes, 0);
        word(&mut bytes, 1);
        for value in [0, 3, 1, 0, 0, 0x807] {
            word(&mut bytes, value);
        }
        floats(&mut bytes, &[0., 0., 0., 1., 0., 0., 0., 1., 0.]);
        for _ in 0..3 {
            bytes.extend([1, bone, 255]);
        }
        for value in [0, 1, last_index] {
            word(&mut bytes, value);
        }
        for value in [0_u16, 0x0040, 0xc040] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend([0xff; 2]);
        for (index, value) in [
            0.42086253, 0.4007645, 0.4432116, 0.98879343, 0.8562466, 0.9024565,
        ]
        .into_iter()
        .enumerate()
        {
            if index != 0 {
                word(&mut bytes, 0x7fc00000);
            }
            floats(&mut bytes, &[value]);
        }
        word(&mut bytes, 0x7fc00000);
        bytes
    }

    #[test]
    fn mesh_reads_split_uv_arrays_and_ignores_stale_cache_words() {
        let mesh = read_mesh(&mesh(0, 2)).unwrap();
        assert_eq!(mesh.sections[0].vertices[1].influences, vec![(0, 255)]);
        assert_eq!(mesh.sections[0].faces[0].indices, [0, 1, 2]);
        assert_eq!(mesh.sections[0].faces[0].normals[0], [0., 0., 1.]);
        assert!(mesh.sections[0].faces[0].normals[1][0] > 0.7);
        assert!(mesh.sections[0].faces[0].normals[2][1] < -0.7);
        assert_eq!(
            mesh.sections[0].faces[0].uv[0],
            [107. * (1. / 255.), 252. * (1. / 255.)]
        );
        assert_eq!(
            mesh.sections[0].faces[0].uv[1],
            [102. * (1. / 255.), 218. * (1. / 255.)]
        );
    }

    #[test]
    fn mesh_rejects_unknown_bones_bad_faces_and_truncation() {
        assert!(read_mesh(&mesh(1, 2)).is_err());
        assert!(read_mesh(&mesh(0, 99)).is_err());
        let mut bytes = mesh(0, 2);
        bytes.pop();
        assert!(read_mesh(&bytes).is_err());
        bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_mesh(&bytes).is_err());
    }

    fn motion(times: &[f32]) -> Vec<u8> {
        let mut bytes = b"anm\0".to_vec();
        for value in [100, 0x7fc00000, 1, 0, 0, u32::MAX] {
            word(&mut bytes, value);
        }
        string(&mut bytes, "Bip01");
        word(&mut bytes, times.len() as u32);
        word(&mut bytes, 3);
        for &time in times {
            floats(&mut bytes, &[time, time, 0., 0., 0., 0., 0., 1.]);
            bytes.extend([0xff; 36]);
        }
        bytes
    }

    #[test]
    fn motion_uses_native_millisecond_truncation_and_validates_keys() {
        let motion = read_motion(&motion(&[0., 1. / 3.])).unwrap();
        assert_eq!(motion.duration_ms, 333);
        assert_eq!(motion.tracks[0].frames[1].time_ms, 333);
        assert!(read_motion(&self::motion(&[1., 0.])).is_err());
        assert!(read_motion(&self::motion(&[f32::NAN])).is_err());
        assert!(read_motion(&self::motion(&[])).is_err());
    }

    #[test]
    fn material_preserves_colors_and_all_texture_slots() {
        let mut bytes = b"mtl\0".to_vec();
        word(&mut bytes, 100);
        word(&mut bytes, 1);
        floats(&mut bytes, &[0.25, 0.5, 0.75, 1., 1., 0.5, 0.25, 1.]);
        bytes.extend([0; 36]);
        for name in ["skin.tga", "", "detail.tga", ""] {
            string(&mut bytes, name);
        }
        let material = read_materials(&bytes).unwrap().remove(0);
        assert_eq!(material.diffuse, [0.25, 0.5, 0.75, 1.]);
        assert_eq!(material.ambient, [1., 0.5, 0.25, 1.]);
        assert_eq!(material.textures, ["skin.tga", "", "detail.tga", ""]);
    }
}
