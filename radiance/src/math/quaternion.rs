use super::Mat44;

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct Quaternion([f32; 4]);

impl Quaternion {
    pub fn new() -> Self {
        Self { 0: [0.; 4] }
    }

    pub fn new_with(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { 0: [x, y, z, w] }
    }

    pub fn to_rotate_matrix(&self) -> Mat44 {
        let x2 = self.0[0] * self.0[0];
        let y2 = self.0[1] * self.0[1];
        let z2 = self.0[2] * self.0[2];
        let xy = self.0[0] * self.0[1];
        let xz = self.0[0] * self.0[2];
        let yz = self.0[1] * self.0[2];
        let wx = self.0[3] * self.0[0];
        let wy = self.0[3] * self.0[1];
        let wz = self.0[3] * self.0[2];

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
