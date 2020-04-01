use crate::director::sce_director::SceCommand;
use crate::resource_manager::ResourceManager;
use crate::scene::{Mv3AnimRepeatMode, Mv3ModelEntity};
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, Entity, Scene};
use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandRoleActive {
    res_man: Rc<ResourceManager>,
}

impl SceCommand for SceCommandRoleActive {
    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
        ui: &mut Ui,
        state: &mut HashMap<String, Box<dyn Any>>,
        delta_sec: f32,
    ) -> bool {
        let mut entity = CoreEntity::new(
            Mv3ModelEntity::new_from_file(
                &self.res_man.mv3_path("101", "C11").to_str().unwrap(),
                Mv3AnimRepeatMode::REPEAT,
            ),
            "ROLE_11",
        );
        entity.load();
        entity
            .transform_mut()
            .set_position(&Vec3::new(-71.1, 0., -71.15));

        scene.entities_mut().push(Box::new(entity));

        return true;
    }
}

impl SceCommandRoleActive {
    pub fn new(res_man: &Rc<ResourceManager>) -> Self {
        Self {
            res_man: res_man.clone(),
        }
    }
}
