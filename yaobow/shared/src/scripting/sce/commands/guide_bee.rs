use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::{IEntityExt, ISceneManager};

use crate::{
    openpal3::{directors::SceneManagerExtensions, scene::ScnScene},
    scripting::sce::{SceCommand, SceState},
};

#[derive(Debug, Clone)]
pub struct SceCommandGuideBee {
    layer: i32,
    x: i32,
    z: i32,
}

impl SceCommandGuideBee {
    pub fn new(layer: i32, x: i32, z: i32) -> Self {
        Self { layer, x, z }
    }
}

impl SceCommand for SceCommandGuideBee {
    fn initialize(&mut self, scene_manager: ComRc<ISceneManager>, state: &mut SceState) {
        if let (Some(scene), Some(leader)) = (
            scene_manager.scn_scene(),
            scene_manager.get_resolved_role(state, -1),
        ) {
            scene.inner::<ScnScene>().launch_bee(
                leader.transform().borrow().position(),
                self.layer,
                self.x,
                self.z,
            );
        }
    }

    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        _state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        true
    }
}
