use super::mat::Mat44;
use super::{vec::Vec3, Quaternion};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Transform {
    mat: Mat44,
}

impl Transform {
    const EPS: f32 = 0.0001;

    pub fn new() -> Self {
        Self {
            mat: Mat44::new_identity(),
        }
    }

    pub fn matrix(&self) -> &Mat44 {
        &self.mat
    }

    pub fn position(&self) -> Vec3 {
        Vec3::new(self.mat[0][3], self.mat[1][3], self.mat[2][3])
    }

    pub fn set_matrix(&mut self, mat: Mat44) -> &mut Self {
        self.mat = mat;
        self
    }

    pub fn set_position(&mut self, pos: &Vec3) -> &mut Self {
        self.mat[0][3] = pos.x;
        self.mat[1][3] = pos.y;
        self.mat[2][3] = pos.z;
        self
    }

    pub fn translate_local(&mut self, vec: &Vec3) -> &mut Self {
        self.translate_impl(vec, true)
    }

    pub fn translate(&mut self, vec: &Vec3) -> &mut Self {
        self.translate_impl(vec, false)
    }

    pub fn rotate_axis_angle_local(&mut self, axis: &Vec3, radian: f32) -> &mut Self {
        self.rotate_axis_angle_impl(axis, radian, true)
    }

    pub fn rotate_axis_angle(&mut self, axis: &Vec3, radian: f32) -> &mut Self {
        self.rotate_axis_angle_impl(axis, radian, false)
    }

    pub fn rotate_quaternion_local(&mut self, quaternion: &Quaternion) -> &mut Self {
        self.rotate_quaternion_impl(quaternion, true)
    }

    pub fn rotate_quaternion(&mut self, quaternion: &Quaternion) -> &mut Self {
        self.rotate_quaternion_impl(quaternion, false)
    }

    pub fn scale_local(&mut self, scale: &Vec3) -> &mut Self {
        self.scale_impl(scale, true)
    }

    pub fn scale(&mut self, scale: &Vec3) -> &mut Self {
        self.scale_impl(scale, false)
    }

    pub fn look_at(&mut self, target: &Vec3) -> &mut Self {
        let position = Vec3::new(self.mat[0][3], self.mat[1][3], self.mat[2][3]);
        let forward = Vec3::normalized(&Vec3::sub(&position, target));
        let right = if (forward.y - 1.).abs() <= Self::EPS
            && forward.x.abs() <= Self::EPS
            && forward.z.abs() <= Self::EPS
        {
            Vec3::new(1., 0., 0.)
        } else {
            Vec3::cross(&Vec3::UP, &forward)
        };

        let up = Vec3::cross(&forward, &right);

        self.mat[0][0] = right.x;
        self.mat[0][1] = up.x;
        self.mat[0][2] = forward.x;

        self.mat[1][0] = right.y;
        self.mat[1][1] = up.y;
        self.mat[1][2] = forward.y;

        self.mat[2][0] = right.z;
        self.mat[2][1] = up.z;
        self.mat[2][2] = forward.z;

        self
    }

    fn translate_impl(&mut self, vec: &Vec3, local: bool) -> &mut Self {
        let mut tm = Mat44::new_identity();
        tm[0][3] = vec.x;
        tm[1][3] = vec.y;
        tm[2][3] = vec.z;

        self.apply(&tm, local)
    }

    fn rotate_axis_angle_impl(&mut self, axis: &Vec3, radian: f32, local: bool) -> &mut Self {
        /*
         *	http://en.wikipedia.org/wiki/Rotation_matrix
         *
         *	c = cos(theta), s = sin(theta), v = (x, y, z), where x^2+y^2+z^2=0
         *	M(v, theta) = [c+(1-c)x^2	(1-c)xy-sz	(1-c)xz+sy]
         *				  [(1-c)xy+sz	c+(1-c)y^2	(1-c)yz-sx]
         *				  [(1-c)zx-sy	(1-c)yz+sx	c+(1-c)z^2]
         *
         */
        let norm = (axis.x * axis.x + axis.y * axis.y + axis.z * axis.z).sqrt();
        if norm == 0. {
            return self;
        }

        let x = axis.x / norm;
        let y = axis.y / norm;
        let z = axis.z / norm;
        let c = radian.cos();
        let s = radian.sin();
        let cp = 1. - c;

        let mut rm = Mat44::new_identity();
        rm[0][0] = c + cp * x * x;
        rm[0][1] = cp * x * y - s * z;
        rm[0][2] = cp * x * z + s * y;
        rm[1][0] = cp * x * y + s * z;
        rm[1][1] = c + cp * y * y;
        rm[1][2] = cp * y * z - s * x;
        rm[2][0] = cp * x * z - s * y;
        rm[2][1] = cp * y * z + s * x;
        rm[2][2] = c + cp * z * z;

        self.apply(&rm, local)
    }

    fn rotate_quaternion_impl(&mut self, quaternion: &Quaternion, local: bool) -> &mut Self {
        self.apply(&quaternion.to_rotate_matrix(), local)
    }

    fn scale_impl(&mut self, scale: &Vec3, local: bool) -> &mut Self {
        let mut mat = Mat44::new_identity();
        mat[0][0] = scale.x;
        mat[1][1] = scale.y;
        mat[2][2] = scale.z;

        self.apply(&mat, local)
    }

    fn apply(&mut self, mat: &Mat44, local: bool) -> &mut Self {
        if local {
            self.mat = Mat44::multiplied(&self.mat, &mat);
        } else {
            self.mat = Mat44::multiplied(&mat, &self.mat);
        }

        self
    }
}
