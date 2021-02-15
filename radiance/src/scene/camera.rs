use crate::math::Mat44;
use crate::math::Transform;

pub struct Camera {
    transform: Transform,
    projection: Mat44,
    fov: f32,
    aspect: f32,
    near_clip: f32,
    far_clip: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self::new_with_params(30. * std::f32::consts::PI / 180., 4. / 3., 0.1, 100000.)
    }

    pub fn new_with_params(fov: f32, aspect: f32, near_clip: f32, far_clip: f32) -> Self {
        Self {
            transform: Transform::new(),
            projection: Self::generate_projection_matrix(fov, aspect, near_clip, far_clip),
            fov,
            aspect,
            near_clip,
            far_clip,
        }
    }

    pub fn set_fov(&mut self, fov: f32) {
        if (fov - self.fov).abs() > std::f32::EPSILON {
            self.fov = fov;
        }

        self.update_projection_matrix();
    }

    pub fn set_aspect(&mut self, aspect: f32) {
        if (aspect - self.aspect).abs() > std::f32::EPSILON {
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
        self.projection =
            Self::generate_projection_matrix(self.fov, self.aspect, self.near_clip, self.far_clip);
    }

    fn generate_projection_matrix(fov: f32, aspect: f32, near_clip: f32, far_clip: f32) -> Mat44 {
        let mut mat = Mat44::new_zero();
        let fti = 1. / (fov / 2.).tan();

        mat[0][0] = if aspect < 1. { fti / aspect } else { fti };
        mat[1][1] = if aspect < 1. { fti } else { fti * aspect };
        mat[2][2] = -(far_clip + near_clip) / (far_clip - near_clip);
        mat[2][3] = -2. * near_clip * far_clip / (far_clip - near_clip);
        mat[3][2] = -1.;

        mat
    }
}
