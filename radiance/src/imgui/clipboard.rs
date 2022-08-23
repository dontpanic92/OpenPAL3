use arboard::Clipboard;
use imgui::ClipboardBackend;

pub struct ClipboardSupport(Clipboard);

pub fn init() -> Option<ClipboardSupport> {
    Clipboard::new().ok().map(ClipboardSupport)
}

impl ClipboardBackend for ClipboardSupport {
    fn get(&mut self) -> Option<String> {
        self.0.get_text().ok().map(|text| text.into())
    }
    fn set(&mut self, text: &str) {
        let _ = self.0.set_text(text.to_owned());
    }
}
