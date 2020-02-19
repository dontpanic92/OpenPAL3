use crate::math::*;

#[derive(Copy, Clone)]
pub struct Vertex {
    position: Vec3,
    tex_coord: Vec2,
}

impl Vertex {
    pub fn new(position: Vec3, tex_coord: Vec2) -> Self {
        Vertex {
            position,
            tex_coord,
        }
    }

    pub fn position(&self) -> &Vec3 {
        &self.position
    }

    pub fn position_mut(&mut self) -> &mut Vec3 {
        &mut self.position
    }

    pub fn tex_coord(&self) -> &Vec2 {
        &self.tex_coord
    }

    pub fn tex_coord_mut(&mut self) -> &mut Vec2 {
        &mut self.tex_coord
    }

    pub fn position_offset() -> usize {
        offset_of!(Vertex, position)
    }

    pub fn tex_coord_offset() -> usize {
        offset_of!(Vertex, tex_coord)
    }
}
