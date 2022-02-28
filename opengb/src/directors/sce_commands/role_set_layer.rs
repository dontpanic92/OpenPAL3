use crate::directors::sce_vm::{SceCommand, SceState};

use crate::directors::SceneManagerExtensions;
use imgui::Ui;
use radiance::scene::SceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandRoleSetLayer {
    role_id: i32,
    layer: i32,
}

impl SceCommand for SceCommandRoleSetLayer {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        scene_manager.resolve_role_mut_do(state, self.role_id, |r| {
            r.set_nav_layer(self.layer as usize)
        });
        true
    }
}

impl SceCommandRoleSetLayer {
    pub fn new(role_id: i32, layer: i32) -> Self {
        Self { role_id, layer }
    }
}
