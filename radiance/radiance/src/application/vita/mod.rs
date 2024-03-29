mod font;
mod screen;

pub struct Platform {}

impl Platform {
    pub fn new() -> Self {
        Self {}
    }

    pub fn initialize(&mut self) {}

    pub fn run_event_loop<F: FnMut()>(&self, mut update_engine: F) {
        loop {
            update_engine();
        }
    }

    pub fn set_title(&self, _: &str) {}

    pub fn dpi_scale(&self) -> f32 {
        1.
    }

    pub fn show_error_dialog(title: &str, msg: &str) {
        log::error!("panic: {}", msg);

        use std::fmt::Write;

        let mut screen = screen::DebugScreen::new();
        let _ = screen.write_str(msg);

        std::thread::sleep(std::time::Duration::from_secs(100));
    }
}
