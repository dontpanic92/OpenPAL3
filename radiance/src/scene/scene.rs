use super::Entity;

pub struct Scene {
    entities: Vec<Entity>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            entities: vec![Entity::new()],
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
