use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;
use radiance::math::Vec3;

use crate::openpal3::directors::SceneManagerExtensions;
use crate::scripting::sce::{SceCommand, SceState};

#[derive(Debug, Clone)]
pub struct SceCommandRoleMoveBack {
    role_id: i32,
    speed: f32,
}

impl SceCommand for SceCommandRoleMoveBack {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        scene_manager.resolve_role_mut_do(state, self.role_id, |e, _r| {
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
