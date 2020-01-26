/// 2d column vector
#[derive(Copy, Clone)]
#[repr(C)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }
}

/// 3d column vector
#[derive(Copy, Clone)]
#[repr(C)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }

    pub fn normalize(&mut self) -> &mut Self {
        *self = Vec3::normalized(self);
        self
    }

    pub fn normalized(vec: &Vec3) -> Self {
        let norm = (vec.x * vec.x + vec.y * vec.y + vec.z * vec.z).sqrt();
        if norm == 0. {
            *vec
        } else {
            Vec3::new(vec.x / norm, vec.y / norm, vec.z / norm)
        }
    }
}

/*// Homogeneous coordinate
#[derive(Copy, Clone)]
#[repr(C)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        if w == 0. {
            Vec4 { x, y, z, w }
        } else {
            Vec4 { x: x / w, y: y / w, z: z / w, w: 1 }
        }
    }
}
*/
