use radiance::math::{Mat44, Vec3};

pub(super) struct OrientedBox {
    center: Vec3,
    axes: [Vec3; 3],
    half: [f32; 3],
}

impl OrientedBox {
    pub fn from_bounds(min: Vec3, max: Vec3, matrix: &Mat44, width_scale: f32) -> Self {
        let m = matrix.floats();
        let local = Vec3::scalar_mul(0.5, &Vec3::add(&min, &max));
        let center = Vec3::new(
            m[0][0] * local.x + m[0][1] * local.y + m[0][2] * local.z + m[0][3],
            m[1][0] * local.x + m[1][1] * local.y + m[1][2] * local.z + m[1][3],
            m[2][0] * local.x + m[2][1] * local.y + m[2][2] * local.z + m[2][3],
        );
        let mut axes = [
            Vec3::new(m[0][0], m[1][0], m[2][0]),
            Vec3::new(m[0][1], m[1][1], m[2][1]),
            Vec3::new(m[0][2], m[1][2], m[2][2]),
        ];
        let mut half = [(max.x - min.x) * width_scale, max.y - min.y, max.z - min.z];
        for i in 0..3 {
            let length = axes[i].norm();
            half[i] *= length * 0.5;
            if length > 0. {
                axes[i] = Vec3::scalar_mul(1. / length, &axes[i]);
            }
        }
        Self { center, axes, half }
    }

    // Original gbOBBox::IsColOBB includes face contact. Test the 15 SAT axes,
    // not the world AABB, so rotated swinging hazards retain their narrow shape.
    pub fn intersects(&self, other: &Self) -> bool {
        let delta = Vec3::sub(&other.center, &self.center);
        let separated = |axis: &Vec3| {
            let radius = |b: &Self| {
                (0..3)
                    .map(|i| b.half[i] * Vec3::dot(&b.axes[i], axis).abs())
                    .sum::<f32>()
            };
            Vec3::dot(&delta, axis).abs() > radius(self) + radius(other)
        };
        for axis in self.axes.iter().chain(other.axes.iter()) {
            if separated(axis) {
                return false;
            }
        }
        for a in &self.axes {
            for b in &other.axes {
                if separated(&Vec3::cross(a, b)) {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radiance::math::Transform;

    fn bounds(position: Vec3, size: Vec3, yaw: f32, scale: f32) -> OrientedBox {
        let mut transform = Transform::new();
        transform
            .set_position(&position)
            .rotate_axis_angle_local(&Vec3::UP, yaw);
        OrientedBox::from_bounds(
            Vec3::scalar_mul(-0.5, &size),
            Vec3::scalar_mul(0.5, &size),
            transform.matrix(),
            scale,
        )
    }

    #[test]
    fn native_obb_cases_include_contact_and_reject_rotated_aabb_false_positives() {
        let origin = Vec3::new(0., 0., 0.);
        let unit = Vec3::new(2., 2., 2.);
        let first = bounds(origin, unit, 0., 1.);
        assert!(first.intersects(&bounds(Vec3::new(2., 0., 0.), unit, 0., 1.)));
        assert!(!first.intersects(&bounds(Vec3::new(2.01, 0., 0.), unit, 0., 1.)));
        let thin = bounds(
            origin,
            Vec3::new(6., 1., 1.),
            std::f32::consts::FRAC_PI_4,
            1.,
        );
        let small = Vec3::new(0.2, 0.2, 0.2);
        assert!(!thin.intersects(&bounds(Vec3::new(2., 0., 2.), small, 0., 1.)));
        assert!(thin.intersects(&bounds(Vec3::new(2., 0., -2.), small, 0., 1.)));
        let actor = bounds(origin, Vec3::new(4., 2., 2.), 0., 0.6);
        assert!(actor.intersects(&bounds(Vec3::new(2.1, 0., 0.), unit, 0., 1.)));
        assert!(!actor.intersects(&bounds(Vec3::new(2.3, 0., 0.), unit, 0., 1.)));
    }
}
