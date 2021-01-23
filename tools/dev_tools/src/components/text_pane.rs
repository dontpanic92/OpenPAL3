use super::ContentPane;
use imgui::{im_str, ImString, InputTextMultiline};
use std::path::PathBuf;

pub struct TextPane {
    text: ImString,
    path: PathBuf,
}

impl TextPane {
    pub fn new(text: String, path: PathBuf) -> Self {
        Self {
            text: im_str!("{}", text),
            path,
        }
    }
}

impl ContentPane for TextPane {
    fn render(&mut self, ui: &imgui::Ui) {
        InputTextMultiline::new(
            ui,
            &im_str!("##text_pane_{:?}", self.path),
            &mut self.text,
            [-1., -1.],
        )
        .read_only(true)
        .build();
    }
}
