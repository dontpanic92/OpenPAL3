use crate::{
    openpal3::directors::SceneManagerExtensions,
    scripting::sce::{SceCommand, SceState},
};

use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandRoleSetLayer {
    role_id: i32,
    layer: i32,
}

impl SceCommand for SceCommandRoleSetLayer {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        scene_manager.resolve_role_mut_do(state, self.role_id, |e, r| {
            r.get().set_nav_layer(self.layer as usize)
        });
        true
    }
}

impl SceCommandRoleSetLayer {
    pub fn new(role_id: i32, layer: i32) -> Self {
        Self { role_id, layer }
    }
}
