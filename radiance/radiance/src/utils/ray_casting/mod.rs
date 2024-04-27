use crate::math::Vec3;

mod mesh;

pub struct RayCaster {
    colliders: Vec<mesh::Mesh>,
}

impl RayCaster {
    pub fn new() -> Self {
        Self {
            colliders: Vec::new(),
        }
    }

    pub fn add_mesh(&mut self, vertices: Vec<Vec3>, indices: Vec<u32>) {
        self.colliders.push(mesh::Mesh::new(vertices, indices));
    }

    pub fn cast_aaray(&self, ray_origin: &Vec3, ray_direction: AARayDirection) -> Option<f32> {
        let mut min_distance = f32::MAX;
        let mut hit = false;

        for collider in &self.colliders {
            if let Some(distance) = collider.cast_aaray(ray_origin, ray_direction) {
                if distance < min_distance {
                    min_distance = distance;
                    hit = true;
                }
            }
        }

        if hit {
            Some(min_distance)
        } else {
            None
        }
    }

    pub fn cast_ray(&self, ray_origin: &Vec3, ray_direction: &Vec3) -> Option<f32> {
        let mut min_distance = f32::MAX;
        let mut hit = false;

        for collider in &self.colliders {
            if let Some(distance) = collider.cast_ray(ray_origin, ray_direction) {
                if distance < min_distance {
                    min_distance = distance;
                    hit = true;
                }
            }
        }

        if hit {
            Some(min_distance)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy)]
pub enum AARayDirection {
    X,
    Y,
    Z,
    NX,
    NY,
    NZ,
}

impl AARayDirection {
    pub fn get_direction(&self) -> Vec3 {
        match self {
            AARayDirection::X => Vec3::new(1., 0., 0.),
            AARayDirection::Y => Vec3::new(0., 1., 0.),
            AARayDirection::Z => Vec3::new(0., 0., 1.),
            AARayDirection::NX => Vec3::new(-1., 0., 0.),
            AARayDirection::NY => Vec3::new(0., -1., 0.),
            AARayDirection::NZ => Vec3::new(0., 0., -1.),
        }
    }
}
