use crate::director::sce_director::SceCommand;
use crate::resource_manager::ResourceManager;
use crate::scene::{Mv3AnimRepeatMode, Mv3ModelEntity};
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, Entity, Scene};
use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandRoleShowAction {
    res_man: Rc<ResourceManager>,
    role_id: i32,
    action_name: String,
    repeat_mode: i32,
}

impl SceCommand for SceCommandRoleShowAction {
    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
        state: &mut HashMap<String, Box<dyn Any>>,
        delta_sec: f32,
    ) -> bool {
        scene.entities_mut().retain(|e| e.name() != "ROLE_11");

        let mut entity = CoreEntity::new(
            Mv3ModelEntity::new_from_file(
                &self.res_man.mv3_path("101", "j04").to_str().unwrap(),
                Mv3AnimRepeatMode::NO_REPEAT,
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

impl SceCommandRoleShowAction {
    pub fn new(
        res_man: &Rc<ResourceManager>,
        role_id: i32,
        action_name: &str,
        repeat_mode: i32,
    ) -> Self {
        Self {
            res_man: res_man.clone(),
            role_id,
            action_name: action_name.to_owned(),
            repeat_mode,
        }
    }
}
