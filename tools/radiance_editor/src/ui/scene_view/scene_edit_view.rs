use imgui::{Condition, Ui};
use radiance::scene::{SceneManager, Viewport};

use crate::ui::window_content_rect;

pub struct SceneEditView {}

impl SceneEditView {
    pub fn render(&mut self, scene_manager: &mut dyn SceneManager, ui: &Ui, _delta_sec: f32) {
        let [window_width, window_height] = ui.window_size();
        ui.window("场景")
            .collapsible(false)
            .resizable(false)
            .size([window_width * 0.6, window_height * 0.7], Condition::Always)
            .position([window_width * 0.2, 0.], Condition::Always)
            .movable(false)
            .draw_background(false)
            .build(|| {
                let rect = window_content_rect(ui);
                scene_manager
                    .scene_mut()
                    .unwrap()
                    .camera_mut()
                    .set_viewport(Viewport::CustomViewport(rect));
            });
    }
}
