use imgui::{Condition, Ui};
use radiance::scene::SceneManager;

pub struct PropertyView {}

impl PropertyView {
    pub fn render(&mut self, _scene_manager: &mut dyn SceneManager, ui: &Ui, _delta_sec: f32) {
        let [window_width, window_height] = ui.window_size();
        ui.window("属性")
            .collapsible(false)
            .resizable(false)
            .size([window_width * 0.2, window_height * 0.7], Condition::Always)
            .position([window_width * 0.8, 0.], Condition::Always)
            .movable(false)
            .build(|| ui.text("hello world"));
    }
}
