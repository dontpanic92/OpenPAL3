use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandDlgFace {
    _id: i32,
    face_name: String,
    left_or_right: i32,
}

impl SceCommand for SceCommandDlgFace {
    fn initialize(&mut self, _scene_manager: ComRc<ISceneManager>, _state: &mut SceState) {}

    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        state
            .dialog_box()
            .set_avator(&self.face_name, self.left_or_right);

        true
    }
}

impl SceCommandDlgFace {
    pub fn new(_id: i32, face_name: String, left_or_right: i32) -> Self {
        Self {
            _id,
            face_name,
            left_or_right,
        }
    }
}
