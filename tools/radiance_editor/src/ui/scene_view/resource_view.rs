use std::ffi::c_void;

use crosscom::ComRc;
use imgui::{Condition, StyleVar, Ui};
use radiance::scene::SceneManager;

use crate::core::IViewContent;

pub struct ResourceView {
    content: Option<ComRc<IViewContent>>,
}

impl ResourceView {
    pub fn new(content: Option<ComRc<IViewContent>>) -> Self {
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
                    let s = scene_manager as *const dyn SceneManager
                        as *const radiance::scene::DefaultSceneManager
                        as *const c_void as i64;
                    let u = ui as *const Ui as i64;
                    content.render(s, u, delta_sec);
                }
            });

        style.pop();
    }
}
