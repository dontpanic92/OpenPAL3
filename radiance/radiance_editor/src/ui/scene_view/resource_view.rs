use crosscom::ComRc;
use imgui::{Condition, StyleVar, Ui};
use radiance::comdef::ISceneManager;

use crate::comdef::IViewContent;

pub struct ResourceView {
    content: Option<ComRc<IViewContent>>,
}

impl ResourceView {
    pub fn new(content: Option<ComRc<IViewContent>>) -> Self {
        Self { content }
    }

    pub fn render(&mut self, _scene_manager: ComRc<ISceneManager>, ui: &Ui, delta_sec: f32) {
        let [window_width, window_height] = ui.window_size();
        let style = ui.push_style_var(StyleVar::WindowPadding([0., 0.]));
        ui.window("资源")
            .collapsible(false)
            .size([window_width, window_height * 0.3], Condition::FirstUseEver)
            .position([0., window_height * 0.7], Condition::FirstUseEver)
            .movable(true)
            .draw_background(false)
            .build(|| {
                if let Some(content) = &mut self.content {
                    content.render(delta_sec);
                }
            });

        style.pop();
    }
}
