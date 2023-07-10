use crate::openpal3::directors::SceneManagerExtensions;
use crate::scripting::sce::{SceCommand, SceState};

use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;
use radiance::math::Vec3;

#[derive(Debug, Clone)]
pub struct SceCommandRoleTurnFace {
    role_id: i32,
    degree: f32,
}

impl SceCommand for SceCommandRoleTurnFace {
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
                .rotate_axis_angle_local(&Vec3::UP, -self.degree.to_radians());
        });
        true
    }
}

impl SceCommandRoleTurnFace {
    pub fn new(role_id: i32, degree: i32) -> Self {
        Self { role_id, degree: degree as f32 }
    }
}
