use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;
use crate::openpal3::directors::SceneManagerExtensions;
use crate::openpal3::scene::RoleController;

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
        let _resolved_role_id = if self._id == -1 {
            state.global_state().role_controlled()
        } else {
            self._id
        };
        let role_entity = _scene_manager
            .scn_scene()
            .unwrap()
            .get()
            .get_role_entity(_resolved_role_id)
            .unwrap();
        let role_name = RoleController::get_role_controller(role_entity.clone()).unwrap().get().model_name();
        state
            .dialog_box()
            .set_avator(&role_name, &self.face_name, self.left_or_right);

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
