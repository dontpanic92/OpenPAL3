use super::director::Director;
use crate::scene::Mv3ModelEntity;
use crate::{resource_manager::ResourceManager, scene::ScnScene};
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, Entity, EntityCallbacks, Scene};
use std::rc::Rc;

pub struct SceDirector {
    res_man: Rc<ResourceManager>,
    init: bool,
}

impl SceDirector {
    pub fn new(res_man: &Rc<ResourceManager>) -> Self {
        Self {
            res_man: res_man.clone(),
            init: false,
        }
    }

    pub fn update(&mut self, scn: &mut Box<dyn Scene>) {
        if self.init == false {
            let mut entity = CoreEntity::new(
                Mv3ModelEntity::new_from_file(
                    &self.res_man.mv3_path("101", "C11").to_str().unwrap(),
                ),
                "ROLE_11",
            );
            entity.load();
            entity
                .transform_mut()
                .set_position(&Vec3::new(-71.1, 0., -71.15));

            scn.entities_mut().push(Box::new(entity));

            self.init = true;
        }
    }
}
