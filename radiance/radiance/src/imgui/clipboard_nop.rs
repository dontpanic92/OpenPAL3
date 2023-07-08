use imgui::ClipboardBackend;

pub struct ClipboardSupport;

pub fn init() -> Option<ClipboardSupport> {
    None
}

impl ClipboardBackend for ClipboardSupport {
    fn get(&mut self) -> Option<String> {
        None
    }

    fn set(&mut self, _: &str) {}
}
