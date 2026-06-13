use super::{Mat44, Vec3};
use serde::{Deserialize, Serialize};

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

    pub fn normalized(q: &Quaternion) -> Self {
        let norm = (q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w).sqrt();
        if norm == 0. {
            *q
        } else {
            Quaternion::new(q.x / norm, q.y / norm, q.z / norm, q.w / norm)
        }
    }

    pub fn lerp(q1: &Self, q2: &Self, pct: f32) -> Self {
        let ret = Self::new(
            q1.x * (1. - pct) + q2.x * pct,
            q1.y * (1. - pct) + q2.y * pct,
            q1.z * (1. - pct) + q2.z * pct,
            q1.w * (1. - pct) + q2.w * pct,
        );

        Self::normalized(&ret)
    }

    pub fn slerp(q1: &Self, q2: &Self, pct: f32) -> Self {
        let mut cos_theta = q1.x * q2.x + q1.y * q2.y + q1.z * q2.z + q1.w * q2.w;

        // Quaternions `q` and `-q` encode the same rotation. When the dot
        // product is negative the two keyframes sit on opposite hemispheres and
        // a naive slerp interpolates the long way around (>180°), swinging the
        // bone far from its intended pose and tearing that mesh part off. Flip
        // one endpoint so we always travel the shortest arc.
        let mut q2 = *q2;
        if cos_theta < 0. {
            q2 = Self::new(-q2.x, -q2.y, -q2.z, -q2.w);
            cos_theta = -cos_theta;
        }

        if cos_theta > 0.999 {
            return Self::lerp(q1, &q2, pct);
        }

        let theta = cos_theta.acos();

        let inv_sin_theta = 1. / theta.sin();
        let t1 = ((1. - pct) * theta).sin() * inv_sin_theta;
        let t2 = (pct * theta).sin() * inv_sin_theta;

        Self::normalized(&Self::new(
            q1.x * t1 + q2.x * t2,
            q1.y * t1 + q2.y * t2,
            q1.z * t1 + q2.z * t2,
            q1.w * t1 + q2.w * t2,
        ))
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
        let w2 = self.w * self.w;
        let xy = self.x * self.y;
        let xz = self.x * self.z;
        let yz = self.y * self.z;
        let wx = self.w * self.x;
        let wy = self.w * self.y;
        let wz = self.w * self.z;

        let mut matrix = Mat44::new_identity();

        matrix[0][0] = 2. * (w2 + x2) - 1.;
        matrix[0][1] = 2. * (xy - wz);
        matrix[0][2] = 2. * (xz + wy);
        matrix[1][0] = 2. * (xy + wz);
        matrix[1][1] = 2. * (w2 + y2) - 1.;
        matrix[1][2] = 2. * (yz - wx);
        matrix[2][0] = 2. * (xz - wy);
        matrix[2][1] = 2. * (yz + wx);
        matrix[2][2] = 2. * (w2 + z2) - 1.;

        matrix
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec3;

    fn rotate(q: &Quaternion, v: &Vec3) -> Vec3 {
        let m = q.to_rotate_matrix();
        Vec3::new(
            v.x * m[0][0] + v.y * m[0][1] + v.z * m[0][2],
            v.x * m[1][0] + v.y * m[1][1] + v.z * m[1][2],
            v.x * m[2][0] + v.y * m[2][1] + v.z * m[2][2],
        )
    }

    #[test]
    fn slerp_takes_shortest_arc_when_endpoints_are_double_covered() {
        let z = Vec3::new(0., 0., 1.);
        let q1 = Quaternion::from_axis_angle(&z, 0.2);
        let q2 = Quaternion::from_axis_angle(&z, 0.6);
        // Same rotation as q2 but the antipodal representation (negative dot
        // with q1) — this is what used to trigger the long-way-around swing.
        let q2_neg = Quaternion::new(-q2.x, -q2.y, -q2.z, -q2.w);

        let mid = Quaternion::slerp(&q1, &q2_neg, 0.5);
        // Halfway between 0.2 and 0.6 rad about Z = 0.4 rad.
        let expected = Quaternion::from_axis_angle(&z, 0.4);

        let v = Vec3::new(1., 0., 0.);
        let got = rotate(&mid, &v);
        let want = rotate(&expected, &v);
        assert!(
            (got.x - want.x).abs() < 1e-3
                && (got.y - want.y).abs() < 1e-3
                && (got.z - want.z).abs() < 1e-3,
            "got {:?}, want {:?}",
            got,
            want,
        );
    }

    #[test]
    fn slerp_antipodal_identity_stays_identity() {
        let id = Quaternion::new(0., 0., 0., 1.);
        let neg_id = Quaternion::new(0., 0., 0., -1.);
        let mid = Quaternion::slerp(&id, &neg_id, 0.5);
        // Both represent no rotation; the blend must not blow up through zero.
        let v = Vec3::new(1., 2., 3.);
        let got = rotate(&mid, &v);
        assert!((got.x - 1.).abs() < 1e-3);
        assert!((got.y - 2.).abs() < 1e-3);
        assert!((got.z - 3.).abs() < 1e-3);
    }
}
