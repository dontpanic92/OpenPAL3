use super::Scene;
use imgui::Ui;

pub trait Director {
    fn update(&mut self, scene: &mut Box<dyn Scene>, ui: &mut Ui, delta_sec: f32);
}
