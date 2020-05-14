use crate::{
    director::{sce_director::SceCommand, sce_state::SceState},
    scene::ScnScene,
};
use imgui::Ui;
use radiance::scene::CoreScene;

#[derive(Clone)]
pub struct SceCommandNop {}

impl SceCommand for SceCommandNop {
    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        true
    }
}

impl SceCommandNop {
    pub fn new() -> Self {
        Self {}
    }
}
