pub struct Platform {}

impl Platform {
    pub fn new() -> Self {
        Self {}
    }

    pub fn show_error_dialog(title: &str, msg: &str) {}

    pub fn initialize(&self) {}

    pub fn process_message(&self) -> bool {
        true
    }

    pub fn set_title(&mut self, title: &str) {}
}
