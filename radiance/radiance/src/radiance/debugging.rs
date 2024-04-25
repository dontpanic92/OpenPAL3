use crosscom::ComRc;
use imgui::Ui;

use crate::comdef::ISceneManager;

pub trait DebugLayer {
    fn update(&self, delta_sec: f32);
}
