use crate::directors::sce_director::{SceCommand, SceState};

use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Clone)]
pub struct SceCommandObjectActive {
    object_id: i32,
    active: i32,
}

impl SceCommand for SceCommandObjectActive {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        if let Some(e) = scene_manager
            .core_scene_mut_or_fail()
            .get_root_object_mut(self.object_id)
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
