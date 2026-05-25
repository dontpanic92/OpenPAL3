mod font;
mod screen;

pub struct Platform {
    quit_requested: std::rc::Rc<std::cell::Cell<bool>>,
}

impl Platform {
    pub fn new() -> Self {
        Self {
            quit_requested: std::rc::Rc::new(std::cell::Cell::new(false)),
        }
    }

    pub fn initialize(&mut self) {}

    pub fn run_event_loop<F: FnMut()>(&self, mut update_engine: F) {
        loop {
            if self.quit_requested.get() {
                break;
            }
            update_engine();
        }
    }

    pub fn request_exit(&self) {
        self.quit_requested.set(true);
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
