use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;
use radiance::math::{Transform, Vec3};

#[derive(Debug, Clone)]
pub struct SceCommandQuake {
    duration: f32,
    amplitude: f32,
    spent: f32,
    original_position: Vec3,
}

impl SceCommand for SceCommandQuake {
    fn initialize(&mut self, scene_manager: ComRc<ISceneManager>, state: &mut SceState) {
        let scene = scene_manager.scene().unwrap();
        self.original_position = scene.camera().borrow().transform().position();
    }

    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        _state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let scene = scene_manager.scene().unwrap();
        let mut cam_pos = self.original_position;
        cam_pos.y += self.amplitude * (1. - 2. * rand::random::<f32>());

        scene
            .camera()
            .borrow_mut()
            .transform_mut()
            .set_position(&cam_pos);

        self.spent += _delta_sec;
        if self.spent > self.duration {
            scene
                .camera()
                .borrow_mut()
                .transform_mut()
                .set_position(&self.original_position);
            true
        } else {
            false
        }
    }
}

impl SceCommandQuake {
    pub fn new(duration: f32, amplitude: f32) -> Self {
        Self {
            duration,
            amplitude,
            spent: 0.,
            original_position: Vec3::new_zeros(),
        }
    }
}
