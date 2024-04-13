use crate::math::Vec3;

use super::AARayDirection;

struct Triangle {
    indices: [u32; 3],
    aabb_min: Vec3,
    aabb_max: Vec3,
}

impl Triangle {
    pub fn new(indices: [u32; 3], vertices: &Vec<Vec3>) -> Self {
        let a = vertices[indices[0] as usize];
        let b = vertices[indices[1] as usize];
        let c = vertices[indices[2] as usize];
        let aabb_min = Vec3::new(
            a.x.min(b.x).min(c.x),
            a.y.min(b.y).min(c.y),
            a.z.min(b.z).min(c.z),
        );
        let aabb_max = Vec3::new(
            a.x.max(b.x).max(c.x),
            a.y.max(b.y).max(c.y),
            a.z.max(b.z).max(c.z),
        );

        Self {
            indices,
            aabb_min,
            aabb_max,
        }
    }

    pub fn cast_aaray(
        &self,
        ray_origin: Vec3,
        aaray: AARayDirection,
        vertices: &Vec<Vec3>,
    ) -> Option<f32> {
        let aabb_min = self.aabb_min;
        let aabb_max = self.aabb_max;
        let ray_direction = aaray.get_direction();

        if ray_direction.x != 0. {
            if ray_origin.y < aabb_min.y
                || ray_origin.y > aabb_max.y
                || ray_origin.z < aabb_min.z
                || ray_origin.z > aabb_max.z
            {
                return None;
            }
        } else if ray_direction.y != 0. {
            if ray_origin.x < aabb_min.x
                || ray_origin.x > aabb_max.x
                || ray_origin.z < aabb_min.z
                || ray_origin.z > aabb_max.z
            {
                return None;
            }
        } else if ray_direction.z != 0. {
            if ray_origin.x < aabb_min.x
                || ray_origin.x > aabb_max.x
                || ray_origin.y < aabb_min.y
                || ray_origin.y > aabb_max.y
            {
                return None;
            }
        }

        self.cast_ray(ray_origin, ray_direction, vertices)
    }

    pub fn cast_ray(
        &self,
        ray_origin: Vec3,
        ray_direction: Vec3,
        vertices: &Vec<Vec3>,
    ) -> Option<f32> {
        const EPSILON: f32 = 0.00001;
        let edge1 = Vec3::sub(
            &vertices[self.indices[1] as usize],
            &vertices[self.indices[0] as usize],
        );
        let edge2 = Vec3::sub(
            &vertices[self.indices[2] as usize],
            &vertices[self.indices[0] as usize],
        );

        let h = Vec3::cross(&ray_direction, &edge2);
        let a = Vec3::dot(&edge1, &h);

        if a > -EPSILON && a < EPSILON {
            // println!("none1");
            return None;
        }

        let f = 1.0 / a;
        let s = Vec3::sub(&ray_origin, &vertices[self.indices[0] as usize]);
        let u = f * Vec3::dot(&s, &h);
        if u < 0.0 || u > 1.0 {
            // println!("none2");
            return None;
        }

        let q = Vec3::cross(&s, &edge1);
        let v = f * Vec3::dot(&ray_direction, &q);

        if v < 0.0 || u + v > 1.0 {
            // println!("none3");
            return None;
        }

        let t = f * Vec3::dot(&edge2, &q);

        if t > EPSILON {
            Some(t)
        } else {
            // println!("none4 t: {}", t);
            None
        }
    }
}

pub(crate) struct Mesh {
    vertices: Vec<Vec3>,
    triangles: Vec<Triangle>,
}

impl Mesh {
    pub fn new(vertices: Vec<Vec3>, indices: Vec<u32>) -> Self {
        let mut triangles = Vec::new();
        for i in (0..indices.len()).step_by(3) {
            triangles.push(Triangle::new(
                [indices[i], indices[i + 1], indices[i + 2]],
                &vertices,
            ));
        }

        Self {
            vertices,
            triangles,
        }
    }

    pub fn cast_aaray(&self, ray_origin: Vec3, aaray: AARayDirection) -> Option<f32> {
        let mut min_distance = None;
        for triangle in &self.triangles {
            if let Some(distance) = triangle.cast_aaray(ray_origin, aaray, &self.vertices) {
                if let Some(md) = min_distance {
                    if distance < md {
                        min_distance = Some(distance);
                    }
                } else {
                    min_distance = Some(distance);
                }
            }
        }

        min_distance
    }

    pub fn cast_ray(&self, ray_origin: Vec3, ray_direction: Vec3) -> Option<f32> {
        let mut min_distance = None;
        for triangle in &self.triangles {
            if let Some(distance) = triangle.cast_ray(ray_origin, ray_direction, &self.vertices) {
                if let Some(md) = min_distance {
                    if distance < md {
                        min_distance = Some(distance);
                    }
                } else {
                    min_distance = Some(distance);
                }
            }
        }

        min_distance
    }
}
