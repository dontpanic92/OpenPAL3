use radiance::math::Vec3;

#[derive(Debug, Clone)]
pub struct Polyline {
    points: Vec<Vec3>,
    lengths: Vec<f32>,
    total: f32,
}

impl Polyline {
    pub fn new(points: &[Vec3]) -> Option<Self> {
        if points.len() < 2
            || points
                .iter()
                .any(|p| !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite())
        {
            return None;
        }
        let mut lengths = Vec::with_capacity(points.len() - 1);
        let mut total = 0.;
        for pair in points.windows(2) {
            let length = Vec3::sub(&pair[1], &pair[0]).norm();
            lengths.push(length);
            total += length;
        }
        (total > 0.).then(|| Self {
            points: points.to_vec(),
            lengths,
            total,
        })
    }

    pub fn total(&self) -> f32 {
        self.total
    }

    pub fn first(&self) -> Vec3 {
        self.points[0]
    }

    pub fn last(&self) -> Vec3 {
        *self.points.last().unwrap()
    }

    pub fn point(&self, distance: f32) -> (Vec3, Vec3) {
        let mut remaining = distance.clamp(0., self.total);
        for (index, length) in self.lengths.iter().copied().enumerate() {
            if remaining <= length || index + 1 == self.lengths.len() {
                let delta = Vec3::sub(&self.points[index + 1], &self.points[index]);
                if length == 0. {
                    remaining -= length;
                    continue;
                }
                return (
                    Vec3::add(
                        &self.points[index],
                        &Vec3::scalar_mul((remaining / length).clamp(0., 1.), &delta),
                    ),
                    Vec3::scalar_mul(1. / length, &delta),
                );
            }
            remaining -= length;
        }
        let tangent = self
            .points
            .windows(2)
            .rev()
            .find_map(|pair| {
                let delta = Vec3::sub(&pair[1], &pair[0]);
                let length = delta.norm();
                (length > 0.).then(|| Vec3::scalar_mul(1. / length, &delta))
            })
            .unwrap_or(Vec3::new(0., 0., 1.));
        (self.last(), tangent)
    }

    pub fn points(&self) -> &[Vec3] {
        &self.points
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polyline_is_linear_and_tolerates_duplicate_points() {
        let path = Polyline::new(&[
            Vec3::new(0., 0., 0.),
            Vec3::new(0., 0., 0.),
            Vec3::new(10., 0., 0.),
            Vec3::new(10., 0., 10.),
        ])
        .unwrap();
        assert_eq!(path.total(), 20.);
        let tuple = |v: Vec3| (v.x, v.y, v.z);
        assert_eq!(tuple(path.point(-1.).0), (0., 0., 0.));
        assert_eq!(tuple(path.point(5.).0), (5., 0., 0.));
        assert_eq!(tuple(path.point(15.).0), (10., 0., 5.));
        assert_eq!(tuple(path.point(30.).0), (10., 0., 10.));
        assert_eq!(tuple(path.point(10.).1), (1., 0., 0.));
    }

    #[test]
    fn polyline_matches_shipped_m17_native_sample() {
        let path = Polyline::new(&[
            Vec3::new(35.58966064453125, 167.72360229492188, -308.958251953125),
            Vec3::new(138.31639099121094, 167.72360229492188, -309.80352783203125),
        ])
        .unwrap();
        assert!((path.total() - 102.73020935058594).abs() < 0.0001);
        let (position, tangent) = path.point(25.682552337646484);
        assert!((position.x - 61.27134323120117).abs() < 0.0001);
        assert!((position.z + 309.1695556640625).abs() < 0.0001);
        assert!((tangent.x - 0.9999661445617676).abs() < 0.0001);
        assert!((tangent.z + 0.008228113874793053).abs() < 0.0001);
    }
}
