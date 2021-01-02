use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::ScnScene;
use imgui::Ui;
use radiance::math::Vec3;
use radiance::scene::{CoreScene, Scene};

#[derive(Clone)]
pub struct SceCommandCameraDefault {}

impl SceCommand for SceCommandCameraDefault {
    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let target = Vec3::new(0., 0., 0.);
        scene
            .camera_mut()
            .transform_mut()
            .set_position(&Vec3::new(300., 300., 300.))
            .look_at(&target);
        return true;
    }
}

impl SceCommandCameraDefault {
    pub fn new(unknown: i32) -> Self {
        Self {}
    }
}
