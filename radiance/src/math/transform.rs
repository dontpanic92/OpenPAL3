use super::mat::Mat44;
use super::vec::Vec3;

pub struct Transform {
    mat: Mat44,
}

impl Transform {
    pub fn new() -> Self {
        Self {
            mat: Mat44::new_identity(),
        }
    }

    pub fn matrix(&self) -> &Mat44 {
        &self.mat
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

    pub fn rotate_local(&mut self, axis: &Vec3, radian: f32) -> &mut Self {
        self.rotate_impl(axis, radian, true)
    }

    pub fn rotate(&mut self, axis: &Vec3, radian: f32) -> &mut Self {
        self.rotate_impl(axis, radian, false)
    }

    fn translate_impl(&mut self, vec: &Vec3, local: bool) -> &mut Self {
        let mut tm = Mat44::new_identity();
        tm[0][3] = vec.x;
        tm[1][3] = vec.y;
        tm[2][3] = vec.z;

        self.apply(&tm, local)
    }

    fn rotate_impl(&mut self, axis: &Vec3, radian: f32, local: bool) -> &mut Self {
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

    fn apply(&mut self, mat: &Mat44, local: bool) -> &mut Self {
        if local {
            self.mat = Mat44::multiplied(&self.mat, &mat);
        } else {
            self.mat = Mat44::multiplied(&mat, &self.mat);
        }

        self
    }
}
