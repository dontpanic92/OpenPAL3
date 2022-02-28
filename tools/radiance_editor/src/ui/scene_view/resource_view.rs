use imgui::{Condition, StyleVar, Ui};
use radiance::scene::SceneManager;

use super::SceneViewSubView;

pub struct ResourceView {
    content: Option<Box<dyn SceneViewSubView>>,
}

impl ResourceView {
    pub fn new(content: Option<Box<dyn SceneViewSubView>>) -> Self {
        Self { content }
    }

    pub fn render(&mut self, scene_manager: &mut dyn SceneManager, ui: &Ui, delta_sec: f32) {
        let [window_width, window_height] = ui.window_size();
        let style = ui.push_style_var(StyleVar::WindowPadding([0., 0.]));
        ui.window("资源")
            .collapsible(false)
            .resizable(false)
            .size([window_width, window_height * 0.3], Condition::Always)
            .position([0., window_height * 0.7], Condition::Always)
            .movable(false)
            .bring_to_front_on_focus(false)
            .draw_background(false)
            .build(|| {
                if let Some(content) = &mut self.content {
                    content.render(scene_manager, ui, delta_sec);
                }
            });

        style.pop();
    }
}
