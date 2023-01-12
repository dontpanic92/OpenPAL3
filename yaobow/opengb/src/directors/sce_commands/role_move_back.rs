use crate::directors::{
    sce_vm::{SceCommand, SceState},
    SceneManagerExtensions,
};
use imgui::Ui;
use radiance::{math::Vec3, scene::SceneManager};

#[derive(Debug, Clone)]
pub struct SceCommandRoleMoveBack {
    role_id: i32,
    speed: f32,
}

impl SceCommand for SceCommandRoleMoveBack {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        scene_manager.resolve_role_mut_do(state, self.role_id, |e, r| {
            e.transform()
                .borrow_mut()
                .translate_local(&Vec3::new(0., 0., self.speed));
        });
        true
    }
}

impl SceCommandRoleMoveBack {
    pub fn new(role_id: i32, speed: f32) -> Self {
        Self { role_id, speed }
    }
}
