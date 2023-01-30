use crate::directors::DevToolsState;

use super::ContentPane;
use imgui::InputTextMultiline;
use std::path::PathBuf;

pub struct TextPane {
    text: String,
    path: PathBuf,
    preview_state: Option<DevToolsState>,
    export: Option<Box<dyn Fn()>>,
}

impl TextPane {
    pub fn new(
        text: String,
        path: PathBuf,
        preview_state: Option<DevToolsState>,
        export: Option<Box<dyn Fn()>>,
    ) -> Self {
        Self {
            text,
            path,
            preview_state,
            export,
        }
    }
}

impl ContentPane for TextPane {
    fn render(&mut self, ui: &imgui::Ui) -> Option<DevToolsState> {
        if self.preview_state.is_some() {
            if ui.button("Preview") {
                return self.preview_state.clone();
            }
        }

        if let Some(export) = &self.export {
            if ui.button("Export") {
                export();
            }
        }

        InputTextMultiline::new(
            ui,
            &format!("##text_pane_{:?}", self.path),
            &mut self.text,
            [-1., -1.],
        )
        .read_only(true)
        .build();

        None
    }
}
