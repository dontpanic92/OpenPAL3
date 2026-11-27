use imgui::Ui;

use super::interp_value::InterpValue;

pub struct ActDrop {
    darkness: InterpValue<f32>,
    color: [f32; 3],
}

impl ActDrop {
    pub fn new() -> Self {
        ActDrop {
            darkness: InterpValue::new(0., 0., 0.),
            color: [0.; 3],
        }
    }

    pub fn set_darkness(&mut self, darkness: InterpValue<f32>) {
        self.darkness = darkness;
        self.color = [0.; 3];
    }

    pub fn set_color(&mut self, color: [f32; 3]) {
        self.color = color;
    }

    pub fn current(&self) -> f32 {
        self.darkness.value()
    }

    pub fn color(&self) -> [f32; 3] {
        self.color
    }

    pub fn update(&mut self, ui: &Ui, delta_sec: f32) {
        self.advance(delta_sec);
        self.draw(ui);
    }

    pub fn advance(&mut self, delta_sec: f32) {
        self.darkness.update(delta_sec);
    }

    pub fn draw(&self, ui: &Ui) {
        let value = self.darkness.value();
        if value == 0. {
            return;
        }

        let [width, height] = ui.io().display_size;
        let color = [self.color[0], self.color[1], self.color[2], value];
        let style = ui.push_style_color(imgui::StyleColor::WindowBg, color);
        ui.window("actdrop")
            .no_decoration()
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
