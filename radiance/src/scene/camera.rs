use crate::math::Mat44;
use crate::math::Transform;

#[cfg(feature = "no_std")]
use micromath::F32Ext;

pub struct Camera {
    transform: Transform,
    projection: Mat44,
    fov43: f32,
    aspect: f32,
    near_clip: f32,
    far_clip: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self::new_with_params(30. * core::f32::consts::PI / 180., 4. / 3., 100., 10000.)
    }

    pub fn new_with_params(fov43: f32, aspect: f32, near_clip: f32, far_clip: f32) -> Self {
        Self {
            transform: Transform::new(),
            projection: Self::generate_projection_matrix(fov43, aspect, near_clip, far_clip),
            fov43,
            aspect,
            near_clip,
            far_clip,
        }
    }

    pub fn set_fov43(&mut self, fov: f32) {
        if (fov - self.fov43).abs() > core::f32::EPSILON {
            self.fov43 = fov;
        }

        self.update_projection_matrix();
    }

    pub fn set_aspect(&mut self, aspect: f32) {
        if (aspect - self.aspect).abs() > core::f32::EPSILON {
            self.aspect = aspect;
        }

        self.update_projection_matrix();
    }

    pub fn transform(&self) -> &Transform {
        &self.transform
    }

    pub fn transform_mut(&mut self) -> &mut Transform {
        &mut self.transform
    }

    pub fn projection_matrix(&self) -> &Mat44 {
        &self.projection
    }

    fn update_projection_matrix(&mut self) {
        self.projection = Self::generate_projection_matrix(
            self.fov43,
            self.aspect,
            self.near_clip,
            self.far_clip,
        );
    }

    fn generate_projection_matrix(fov43: f32, aspect: f32, near_clip: f32, far_clip: f32) -> Mat44 {
        let fov = Self::calculate_fov(fov43, aspect);
        let mut mat = Mat44::new_zero();
        let fti = 1. / (fov / 2.).tan();

        mat[0][0] = if aspect < 1. { fti / aspect } else { fti };
        mat[1][1] = if aspect < 1. { fti } else { fti * aspect };
        mat[2][2] = -(far_clip + near_clip) / (far_clip - near_clip);
        mat[2][3] = -2. * near_clip * far_clip / (far_clip - near_clip);
        mat[3][2] = -1.;

        mat
    }

    fn calculate_fov(fov43: f32, aspect: f32) -> f32 {
        if aspect > 1. {
            fov43 / 4. * 3. * aspect
        } else {
            fov43
        }
    }
}
