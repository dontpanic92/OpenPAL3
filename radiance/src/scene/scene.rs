use super::Entity;
use crate::math::{Vec2, Vec3};
use crate::rendering::{RenderObject, Vertex};

pub struct Scene {
    entities: Vec<Entity>,
}

impl Scene {
    pub fn new() -> Self {
        let mut entity1 = Entity::new();
        entity1.add_component(RenderObject::new_with_data(
            vec![
                Vertex::new(
                    Vec3::new(-0.5, -0.5, 0.),
                    Vec3::new(1., 0., 0.),
                    Vec2::new(0., 1.),
                ),
                Vertex::new(
                    Vec3::new(0.5, -0.5, 0.),
                    Vec3::new(0., 1., 0.),
                    Vec2::new(1., 1.),
                ),
                Vertex::new(
                    Vec3::new(0.5, 0.5, 0.),
                    Vec3::new(0., 0., 1.),
                    Vec2::new(1., 0.),
                ),
                Vertex::new(
                    Vec3::new(-0.5, 0.5, 0.),
                    Vec3::new(1., 1., 1.),
                    Vec2::new(0., 0.),
                ),
            ],
            vec![0, 1, 2, 2, 3, 0],
        ));

        let mut entity2 = Entity::new();
        entity2.add_component(RenderObject::new_with_data(
            vec![
                Vertex::new(
                    Vec3::new(-0.5, -0.5, -1.),
                    Vec3::new(1., 0., 0.),
                    Vec2::new(0., 1.),
                ),
                Vertex::new(
                    Vec3::new(0.5, -0.5, -1.),
                    Vec3::new(0., 1., 0.),
                    Vec2::new(1., 1.),
                ),
                Vertex::new(
                    Vec3::new(0.5, 0.5, -1.),
                    Vec3::new(0., 0., 1.),
                    Vec2::new(1., 0.),
                ),
                Vertex::new(
                    Vec3::new(-0.5, 0.5, -1.),
                    Vec3::new(1., 1., 1.),
                    Vec2::new(0., 0.),
                ),
            ],
            vec![0, 1, 2, 2, 3, 0],
        ));

        Self {
            entities: vec![entity1, entity2],
        }
    }

    pub fn load(&mut self) {}

    pub fn update(&mut self) {}

    pub fn unload(&mut self) {}

    pub fn entities(&self) -> &Vec<Entity> {
        &self.entities
    }

    pub fn entities_mut(&mut self) -> &mut Vec<Entity> {
        &mut self.entities
    }
}
