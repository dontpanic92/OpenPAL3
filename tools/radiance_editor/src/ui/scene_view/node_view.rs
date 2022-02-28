use imgui::{Condition, Ui};
use radiance::scene::SceneManager;

pub struct NodeView {}

impl NodeView {
    pub fn render(&mut self, _scene_manager: &mut dyn SceneManager, ui: &Ui, _delta_sec: f32) {
        let [window_width, window_height] = ui.window_size();
        ui.window("对象树")
            .collapsible(false)
            .resizable(false)
            .size([window_width * 0.2, window_height * 0.7], Condition::Always)
            .position([0., 0.], Condition::Always)
            .movable(false)
            .build(|| ui.text("hello world"));
    }
}
