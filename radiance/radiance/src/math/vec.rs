use serde::{Deserialize, Serialize};

use super::Mat44;

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

    pub fn u8_slice_len() -> usize {
        std::mem::size_of::<Self>()
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

impl From<[f32; 3]> for Vec3 {
    fn from(value: [f32; 3]) -> Self {
        Self {
            x: value[0],
            y: value[1],
            z: value[2],
        }
    }
}

impl Vec3 {
    pub const UP: Self = Vec3 {
        x: 0.,
        y: 1.,
        z: 0.,
    };

    pub const EAST: Self = Vec3 {
        x: 1.,
        y: 0.,
        z: 0.,
    };

    pub const BACK: Self = Vec3 {
        x: 0.,
        y: 0.,
        z: 1.,
    };

    pub const FRONT: Self = Vec3 {
        x: 0.,
        y: 0.,
        z: -1.,
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

    pub fn lerp(v1: &Vec3, v2: &Vec3, pct: f32) -> Self {
        Self::new(
            v1.x * (1. - pct) + v2.x * pct,
            v1.y * (1. - pct) + v2.y * pct,
            v1.z * (1. - pct) + v2.z * pct,
        )
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

    pub fn crossed_mat(lhs: &Vec3, rhs: &Mat44) -> Self {
        /*let x = lhs.x * rhs[0][0] + lhs.y * rhs[1][0] + lhs.z * rhs[2][0] + 1. * rhs[3][0];
        let y = lhs.y * rhs[0][1] + lhs.y * rhs[1][1] + lhs.z * rhs[2][1] + 1. * rhs[3][1];
        let z = lhs.z * rhs[0][2] + lhs.y * rhs[1][2] + lhs.z * rhs[2][2] + 1. * rhs[3][2];
        let mut w = 1. * rhs[0][3] + 1. * rhs[1][3] + 1. * rhs[2][3] + 1. * rhs[3][3];
        if w == 0.0 {
            w = 1.0;
        }*/

        let x = lhs.x * rhs[0][0] + lhs.y * rhs[0][1] + lhs.z * rhs[0][2] + 1. * rhs[0][3];
        let y = lhs.x * rhs[1][0] + lhs.y * rhs[1][1] + lhs.z * rhs[1][2] + 1. * rhs[1][3];
        let z = lhs.x * rhs[2][0] + lhs.y * rhs[2][1] + lhs.z * rhs[2][2] + 1. * rhs[2][3];
        let mut w = 1. * rhs[3][0] + 1. * rhs[3][1] + 1. * rhs[3][2] + 1. * rhs[3][3];
        if w == 0.0 {
            w = 1.0;
        }

        Vec3::new(x / w, y / w, z / w)
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

    pub fn u8_slice_len() -> usize {
        std::mem::size_of::<Self>()
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
