use crate::math::Mat44;
use crate::math::Rect;
use crate::math::Transform;

#[derive(Copy, Clone)]
pub enum Viewport {
    FullExtent(Rect),
    CustomViewport(Rect),
}

pub struct Camera {
    transform: Transform,
    viewport: Viewport,
    projection: Mat44,
    fov43: f32,
    aspect: f32,
    near_clip: f32,
    far_clip: f32,
}

/// Six-plane world-space view frustum.
///
/// Planes are stored as `[nx, ny, nz, d]` with the inside-positive
/// convention `n · p + d >= 0`. Normals are unit-length so `d` is
/// a signed distance (units = world space).
#[derive(Copy, Clone, Debug)]
pub struct Frustum {
    pub planes: [[f32; 4]; 6],
}

impl Camera {
    pub fn new() -> Self {
        Self::new_with_params(30. * std::f32::consts::PI / 180., 4. / 3., 10., 100000.)
    }

    pub fn new_with_params(fov43: f32, aspect: f32, near_clip: f32, far_clip: f32) -> Self {
        Self {
            transform: Transform::new(),
            viewport: Viewport::FullExtent(Rect::new(0., 0., 0., 0.)),
            projection: Self::generate_projection_matrix(fov43, aspect, near_clip, far_clip),
            fov43,
            aspect,
            near_clip,
            far_clip,
        }
    }

    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }

    pub fn set_fov43(&mut self, fov: f32) {
        if (fov - self.fov43).abs() > std::f32::EPSILON {
            self.fov43 = fov;
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

    /// Vertical-reference field of view (4:3 basis) in radians, as supplied to
    /// the projection. Used by the shadow system to reconstruct the view
    /// frustum's split corners.
    pub fn fov43(&self) -> f32 {
        self.fov43
    }

    pub fn aspect(&self) -> f32 {
        self.aspect
    }

    pub fn near_clip(&self) -> f32 {
        self.near_clip
    }

    pub fn far_clip(&self) -> f32 {
        self.far_clip
    }

    /// Build a six-plane view frustum in world space from the camera's
    /// current view + projection. Each plane is stored as
    /// `[nx, ny, nz, d]` with the inside-positive convention
    /// `n · p + d >= 0` after normalization.
    ///
    /// Planes are extracted from `proj * view` using the Gribb-Hartmann
    /// method. The engine's projection puts the near plane at NDC
    /// `z = -1` (OpenGL convention — see `generate_projection_matrix`)
    /// so the near plane is `row3 + row2`; switching the projection
    /// to Vulkan `[0, 1]` NDC later would mean using `row2` alone
    /// here.
    pub fn frustum(&self) -> Frustum {
        let view = Mat44::inversed(&self.transform.matrix());
        let m = Mat44::multiplied(&self.projection, &view);
        let r = m.floats();

        // Row order: [left, right, bottom, top, near, far].
        let raw = [
            [
                r[3][0] + r[0][0],
                r[3][1] + r[0][1],
                r[3][2] + r[0][2],
                r[3][3] + r[0][3],
            ],
            [
                r[3][0] - r[0][0],
                r[3][1] - r[0][1],
                r[3][2] - r[0][2],
                r[3][3] - r[0][3],
            ],
            [
                r[3][0] + r[1][0],
                r[3][1] + r[1][1],
                r[3][2] + r[1][2],
                r[3][3] + r[1][3],
            ],
            [
                r[3][0] - r[1][0],
                r[3][1] - r[1][1],
                r[3][2] - r[1][2],
                r[3][3] - r[1][3],
            ],
            [
                r[3][0] + r[2][0],
                r[3][1] + r[2][1],
                r[3][2] + r[2][2],
                r[3][3] + r[2][3],
            ],
            [
                r[3][0] - r[2][0],
                r[3][1] - r[2][1],
                r[3][2] - r[2][2],
                r[3][3] - r[2][3],
            ],
        ];

        let mut planes = [[0.0f32; 4]; 6];
        for (i, p) in raw.iter().enumerate() {
            let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            let inv = if len > f32::EPSILON { 1.0 / len } else { 1.0 };
            planes[i] = [p[0] * inv, p[1] * inv, p[2] * inv, p[3] * inv];
        }
        Frustum { planes }
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
