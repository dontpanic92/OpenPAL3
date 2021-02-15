use serde::{Serialize, Deserialize};
use std::ops::{Index, IndexMut};
use std::ptr::swap;

/// Row major 4x4 matrix
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct Mat44([[f32; 4]; 4]);

impl Mat44 {
    pub fn new_zero() -> Self {
        Mat44([[0.; 4]; 4])
    }

    pub fn new_identity() -> Self {
        Mat44([
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ])
    }

    pub fn multiply(&mut self, rhs: &Mat44) -> &mut Self {
        *self = Mat44::multiplied(self, rhs);
        self
    }

    pub fn inverse(&mut self) -> &mut Self {
        *self = Mat44::inversed(self);
        self
    }

    pub fn inversed(mat: &Mat44) -> Mat44 {
        /*
         * http://www.cg.info.hiroshima-cu.ac.jp/~miyazaki/knowledge/teche53.html
         *
         */
        let mut inversed = *mat;
        unsafe {
            swap(&mut inversed[0][1], &mut inversed[1][0]);
            swap(&mut inversed[0][2], &mut inversed[2][0]);
            swap(&mut inversed[1][2], &mut inversed[2][1]);
        }

        let mut translate = [0.; 3];
        for i in 0..3usize {
            translate[i] = (0..3usize).map(|j| inversed[i][j] * mat[j][3]).sum();
        }

        inversed[0][3] = -translate[0];
        inversed[1][3] = -translate[1];
        inversed[2][3] = -translate[2];
        inversed
    }

    pub fn multiplied(lhs: &Mat44, rhs: &Mat44) -> Mat44 {
        let mut new_mat = Mat44::new_zero();
        for i in 0..4usize {
            for j in 0..4usize {
                new_mat.0[i][j] = (0..4usize).map(|k| lhs.0[i][k] * rhs.0[k][j]).sum();
            }
        }

        new_mat
    }

    pub fn floats_mut(&mut self) -> &mut [[f32; 4]; 4] {
        &mut self.0
    }
}

impl Index<usize> for Mat44 {
    type Output = [f32; 4];

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for Mat44 {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl std::fmt::Display for Mat44 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in 0..4usize {
            for j in 0..4usize {
                write!(f, "{} ", self.0[i][j])?;
            }
            write!(f, "\n")?;
        }

        write!(f, "\n")
    }
}
