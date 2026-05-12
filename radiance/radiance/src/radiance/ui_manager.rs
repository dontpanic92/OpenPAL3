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
