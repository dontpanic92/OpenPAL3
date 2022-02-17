use crate::application::Platform;

pub struct ImguiContext {}

impl ImguiContext {
    pub fn new(platform: &Platform) -> Self {
        Self {}
    }

    pub fn draw_ui<F: FnOnce(&mut Ui)>(&mut self, delta_sec: f32, draw: F) -> ImguiFrame {
        ImguiFrame {}
    }
}

#[derive(Clone, Copy)]
pub struct TextureId {}

pub struct Ui {}

#[derive(Clone, Copy)]
pub struct ImguiFrame {}
