use crate::{
    openpal3::directors::SceneManagerExtensions,
    scripting::sce::{SceCommand, SceState},
};

use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandObjectActive {
    object_id: i32,
    active: i32,
}

impl SceCommand for SceCommandObjectActive {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        if let Some(e) = scene_manager
            .scn_scene()
            .unwrap()
            .get()
            .get_root_object(self.object_id)
        {
            e.set_visible(self.active != 0);
        }

        true
    }
}

impl SceCommandObjectActive {
    pub fn new(object_id: i32, active: i32) -> Self {
        Self { object_id, active }
    }
}
