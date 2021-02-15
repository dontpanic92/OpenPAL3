use serde::{Serialize, Deserialize};

/// 2d column vector
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self as *const _ as *const u8, std::mem::size_of::<Self>())
        }
    }
}

/// 3d column vector
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const UP: Self = Vec3 {
        x: 0.,
        y: 1.,
        z: 0.,
    };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }

    pub fn new_zeros() -> Self {
        Vec3 {
            x: 0.,
            y: 0.,
            z: 0.,
        }
    }

    pub fn normalize(&mut self) -> &mut Self {
        *self = Vec3::normalized(self);
        self
    }

    pub fn neg(&mut self) -> &mut Self {
        self.x = -self.x;
        self.y = -self.y;
        self.z = -self.z;
        self
    }

    pub fn norm(&self) -> f32 {
        self.norm2().sqrt()
    }

    pub fn norm2(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn add(lhs: &Vec3, rhs: &Vec3) -> Self {
        Vec3::new(lhs.x + rhs.x, lhs.y + rhs.y, lhs.z + rhs.z)
    }

    pub fn sub(lhs: &Vec3, rhs: &Vec3) -> Self {
        Vec3::new(lhs.x - rhs.x, lhs.y - rhs.y, lhs.z - rhs.z)
    }

    pub fn dot(lhs: f32, rhs: &Vec3) -> Self {
        Vec3::new(lhs * rhs.x, lhs * rhs.y, lhs * rhs.z)
    }

    pub fn cross(lhs: &Vec3, rhs: &Vec3) -> Self {
        Vec3::new(
            lhs.y * rhs.z - lhs.z * rhs.y,
            lhs.z * rhs.x - lhs.x * rhs.z,
            lhs.x * rhs.y - lhs.y * rhs.x,
        )
    }

    pub fn normalized(vec: &Vec3) -> Self {
        let norm = (vec.x * vec.x + vec.y * vec.y + vec.z * vec.z).sqrt();
        if norm == 0. {
            *vec
        } else {
            Vec3::new(vec.x / norm, vec.y / norm, vec.z / norm)
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self as *const _ as *const u8, std::mem::size_of::<Self>())
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
