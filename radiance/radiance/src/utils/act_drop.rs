use imgui::Ui;

use super::interp_value::InterpValue;

pub struct ActDrop {
    darkness: InterpValue<f32>,
}

impl ActDrop {
    pub fn new() -> Self {
        ActDrop {
            darkness: InterpValue::new(0., 0., 0.),
        }
    }

    pub fn set_darkness(&mut self, darkness: InterpValue<f32>) {
        self.darkness = darkness;
    }

    pub fn current(&self) -> f32 {
        self.darkness.value()
    }

    pub fn update(&mut self, ui: &Ui, delta_sec: f32) {
        self.darkness.update(delta_sec);

        let value = self.darkness.value();
        if value == 0. {
            return;
        }

        let [width, height] = ui.io().display_size;
        let color = [0., 0., 0., value];
        let style = ui.push_style_color(imgui::StyleColor::WindowBg, color);
        ui.window("actdrop")
            .position([0., 0.], imgui::Condition::Always)
            .size([width, height], imgui::Condition::Always)
            .movable(false)
            .resizable(false)
            .collapsible(false)
            .title_bar(false)
            .draw_background(true)
            .scroll_bar(false)
            .nav_focus(false)
            .focused(false)
            .mouse_inputs(false)
            .build(|| {});
        style.pop();
    }
}
