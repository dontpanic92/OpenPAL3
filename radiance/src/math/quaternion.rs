use super::{Mat44, Vec3};
use serde::{Serialize, Deserialize};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quaternion {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn new_zeros() -> Self {
        Self {
            x: 0.,
            y: 0.,
            z: 0.,
            w: 0.,
        }
    }

    pub fn from_axis_angle(axis: &Vec3, angle: f32) -> Self {
        let half = angle / 2.;
        let sin_half = half.sin();
        let cos_half = half.cos();

        Self {
            x: axis.x * sin_half,
            y: axis.y * sin_half,
            z: axis.z * sin_half,
            w: cos_half,
        }
    }

    pub fn inverse(&mut self) -> &mut Self {
        self.x = -self.x;
        self.y = -self.y;
        self.z = -self.z;

        self
    }

    pub fn to_rotate_matrix(&self) -> Mat44 {
        let x2 = self.x * self.x;
        let y2 = self.y * self.y;
        let z2 = self.z * self.z;
        let xy = self.x * self.y;
        let xz = self.x * self.z;
        let yz = self.y * self.z;
        let wx = self.w * self.x;
        let wy = self.w * self.y;
        let wz = self.w * self.z;

        let mut matrix = Mat44::new_identity();

        matrix[0][0] = 1. - 2. * (y2 + z2);
        matrix[0][1] = 2. * (xy + wz);
        matrix[0][2] = 2. * (xz - wy);

        matrix[1][0] = 2. * (xy - wz);
        matrix[1][1] = 1. - 2. * (x2 + z2);
        matrix[1][2] = 2. * (yz + wx);

        matrix[2][0] = 2. * (xz + wy);
        matrix[2][1] = 2. * (yz - wx);
        matrix[2][2] = 1. - 2. * (x2 + y2);

        matrix
    }
}
