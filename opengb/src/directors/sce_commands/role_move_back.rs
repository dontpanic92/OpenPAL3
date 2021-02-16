use crate::directors::{
    sce_director::{SceCommand, SceState},
    SceneManagerExtensions,
};
use imgui::Ui;
use radiance::{
    math::Vec3,
    scene::{Entity, SceneManager},
};

#[derive(Clone)]
pub struct SceCommandRoleMoveBack {
    role_id: i32,
    speed: f32,
}

impl SceCommand for SceCommandRoleMoveBack {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        scene_manager
            .core_scene_mut_or_fail()
            .get_role_entity_mut(self.role_id)
            .transform_mut()
            .translate_local(&Vec3::new(0., 0., self.speed));
        true
    }
}

impl SceCommandRoleMoveBack {
    pub fn new(role_id: i32, speed: f32) -> Self {
        Self { role_id, speed }
    }
}
