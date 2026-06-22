use std::{cell::RefCell, rc::Rc};

use imgui::Ui;

use crate::{
    application::Platform,
    imgui::{ImguiContext, ImguiFrame},
};

pub struct UiManager {
    imgui_context: Rc<ImguiContext>,
    ui: RefCell<Option<&'static Ui>>,
    dpi_scale: f32,
}

impl UiManager {
    pub fn new(platform: &mut Platform) -> Self {
        let dpi_scale = platform.dpi_scale();
        Self {
            imgui_context: Rc::new(ImguiContext::new(platform)),
            ui: RefCell::new(None),
            dpi_scale,
        }
    }

    pub fn imgui_context(&self) -> Rc<ImguiContext> {
        self.imgui_context.clone()
    }

    /// Register a game-shipped font (raw TTF/TTC bytes) for in-game text.
    /// Appended as an extra atlas slot; the bundled editor/title font is
    /// left in place. `extra_scale` is a per-game size multiplier applied on
    /// top of the per-face normalization. See [`ImguiContext::add_game_font`].
    pub fn add_game_font(&self, data: &[u8], extra_scale: f32) {
        self.imgui_context.add_game_font(data, extra_scale);
    }

    /// `FontId` of the registered game font for the given
    /// [`crate::imgui::GameFontSize`] slot, or `None` if none registered.
    pub fn game_font(&self, size: usize) -> Option<imgui::FontId> {
        self.imgui_context.game_font(size)
    }

    pub fn update(&self, delta_sec: f32, draw_func: impl Fn(&Ui)) -> ImguiFrame {
        let context = self.imgui_context.clone();
        let frame = context.draw_ui(delta_sec, |ui| {
            // Leak it. This is safe because we're only using it for the duration of this function.
            self.ui.replace(unsafe { Some(&*(ui as *const Ui)) });
            draw_func(ui);
            self.ui.replace(None);
        });

        frame
    }

    pub fn ui(&self) -> &'static Ui {
        self.ui
            .borrow()
            .expect("UI is not available outside of the update function")
    }

    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }
}
