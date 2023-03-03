use crosscom::ComRc;
use imgui::{Condition, Ui};
use radiance::comdef::ISceneManager;

pub struct PropertyView {}

impl PropertyView {
    pub fn render(&mut self, _scene_manager: ComRc<ISceneManager>, ui: &Ui, _delta_sec: f32) {
        let [window_width, window_height] = ui.window_size();
        ui.window("属性")
            .collapsible(false)
            .size(
                [window_width * 0.2, window_height * 0.7],
                Condition::Appearing,
            )
            .position([window_width * 0.8, 0.], Condition::Appearing)
            .movable(true)
            .build(|| ui.text("hello world"));
    }
}
