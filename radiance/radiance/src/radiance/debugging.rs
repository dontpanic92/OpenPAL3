use crosscom::ComRc;
use imgui::Ui;

use crate::comdef::ISceneManager;

pub trait DebugLayer {
    fn update(&self, scene_manager: ComRc<ISceneManager>, ui: &Ui, delta_sec: f32);
}
