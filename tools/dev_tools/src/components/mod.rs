use imgui::Ui;

mod audio_pane;
mod text_pane;

pub use audio_pane::AudioPane;
pub use text_pane::TextPane;

pub trait ContentPane {
    fn render(&mut self, ui: &Ui);
}
