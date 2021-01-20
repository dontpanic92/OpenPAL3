use imgui::Ui;

mod audio_pane;

pub use audio_pane::AudioPane;

pub trait ContentPane {
    fn render(&mut self, ui: &Ui);
}
