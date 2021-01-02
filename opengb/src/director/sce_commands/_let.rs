use crate::{
    director::{sce_director::SceCommand, sce_state::SceState},
    scene::ScnScene,
};
use imgui::Ui;
use radiance::scene::CoreScene;

#[derive(Clone)]
pub struct SceCommandLet {
    var: i16,
    value: i32,
}

impl SceCommand for SceCommandLet {
    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        state
            .ext_mut()
            .insert(self.var.to_string(), Box::new(self.value));
        true
    }
}

impl SceCommandLet {
    pub fn new(var: i16, value: i32) -> Self {
        Self { var, value }
    }
}
