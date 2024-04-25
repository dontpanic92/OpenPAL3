use crate::{
    math::{Vec2, Vec3},
    rendering::{MaterialDef, VertexBuffer, VertexComponents},
};

#[derive(Copy, Clone)]
pub struct TexCoord {
    pub u: f32,
    pub v: f32,
}

impl TexCoord {
    pub fn new(u: f32, v: f32) -> Self {
        Self { u, v }
    }
}

#[derive(Clone)]
pub struct Geometry {
    pub material: MaterialDef,
    pub vertices: VertexBuffer,
    pub indices: Vec<u32>,
}

impl Geometry {
    pub fn new(
        vertices: &[Vec3],
        normals: Option<&[Vec3]>,
        texcoords: &[Vec<TexCoord>],
        indices: Vec<u32>,
        material: MaterialDef,
    ) -> Self {
        let mut components = VertexComponents::POSITION;

        if texcoords.len() == 1 {
            components |= VertexComponents::TEXCOORD;
        } else if texcoords.len() == 2 {
            components |= VertexComponents::TEXCOORD | VertexComponents::TEXCOORD2;
        };

        if normals.is_some() {
            components |= VertexComponents::NORMAL;
        }

        let mut buffer = VertexBuffer::new(components, vertices.len());

        for i in 0..vertices.len() {
            let vert = &vertices[i];
            let normal = normals.and_then(|n| Some(&n[i]));

            let texcoord1 = if texcoords.len() > 0 {
                Some(Vec2::new(texcoords[0][i].u, texcoords[0][i].v))
            } else {
                None
            };

            let texcoord2 = if texcoords.len() > 1 {
                Some(Vec2::new(texcoords[1][i].u, texcoords[1][i].v))
            } else {
                None
            };

            buffer.set_data(
                i,
                Some(&Vec3::new(vert.x, vert.y, vert.z)),
                normal,
                texcoord1.as_ref(),
                texcoord2.as_ref(),
            );
        }

        Self {
            material,
            vertices: buffer,
            indices,
        }
    }
}
